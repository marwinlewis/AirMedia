//! Background ffmpeg clip export job with progress reporting and cancellation.

use crate::clip::{ClipExportMode, ClipRange};
use std::io::{BufRead, BufReader, Read};
use std::path::{Path, PathBuf};
use std::process::{Child, Stdio};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::thread::JoinHandle;

/// Status returned by polling an export job.
#[derive(Debug)]
pub enum ClipExportPoll {
    Running { progress: f32 },
    Success(PathBuf),
    Failed(String),
    Cancelled,
}

/// A background ffmpeg process exporting a video clip.
pub struct ClipExportJob {
    pub output_path: PathBuf,
    child: Arc<Mutex<Option<Child>>>,
    cancelled: Arc<AtomicBool>,
    /// Set once ffmpeg has exited and the worker has decided the outcome.
    finished: Arc<AtomicBool>,
    progress: Arc<Mutex<f32>>,
    result: Arc<Mutex<Option<Result<PathBuf, String>>>>,
    worker: Option<JoinHandle<()>>,
}

impl ClipExportJob {
    /// Spawns ffmpeg in the background to cut `range` from `input` into `output`.
    pub fn start(
        input: &Path,
        output: &Path,
        range: ClipRange,
        mode: ClipExportMode,
        include_audio: bool,
    ) -> Result<Self, String> {
        let args = crate::clip::build_clip_args(input, output, range, mode, include_audio);
        let mut cmd = crate::media::create_hidden_command("ffmpeg");
        cmd.args(&args);
        cmd.stdout(Stdio::piped()).stderr(Stdio::piped()).stdin(Stdio::null());

        let mut child = cmd.spawn().map_err(|e| format!("Failed to run ffmpeg: {e}"))?;
        let stdout = child.stdout.take();
        let stderr = child.stderr.take();

        let child_arc = Arc::new(Mutex::new(Some(child)));
        let cancelled = Arc::new(AtomicBool::new(false));
        let finished = Arc::new(AtomicBool::new(false));
        let progress = Arc::new(Mutex::new(0.0f32));
        let result = Arc::new(Mutex::new(None));

        let child_clone = Arc::clone(&child_arc);
        let cancelled_clone = Arc::clone(&cancelled);
        let finished_clone = Arc::clone(&finished);
        let progress_clone = Arc::clone(&progress);
        let result_clone = Arc::clone(&result);
        let output_path = output.to_path_buf();
        let out_path_worker = output_path.clone();
        let clip_dur = range.duration();

        let worker = std::thread::spawn(move || {
            // Drain stderr in background to prevent process pipe buffer saturation
            let err_handle = std::thread::spawn(move || {
                let mut buf = Vec::new();
                if let Some(mut err) = stderr {
                    let _ = err.read_to_end(&mut buf);
                }
                buf
            });

            // Parse stdout key=value lines from `-progress pipe:1`
            if let Some(out) = stdout {
                let reader = BufReader::new(out);
                for line in reader.lines() {
                    if cancelled_clone.load(Ordering::SeqCst) {
                        break;
                    }
                    if let Ok(line) = line {
                        if let Some(val) = line.strip_prefix("out_time_us=") {
                            if let Ok(us) = val.trim().parse::<i64>() {
                                if clip_dur > 0.0 {
                                    let secs = us.max(0) as f64 / 1_000_000.0;
                                    let frac = (secs / clip_dur).clamp(0.0, 1.0) as f32;
                                    if let Ok(mut p) = progress_clone.lock() {
                                        *p = frac;
                                    }
                                }
                            }
                        }
                    }
                }
            }

            // Wait for ffmpeg to exit. Poll with try_wait and release the lock between polls so
            // cancel() can always take it to kill the process.
            let wait_status = loop {
                let polled = match child_clone.lock() {
                    Ok(mut guard) => match guard.as_mut() {
                        Some(c) => c.try_wait(),
                        None => return,
                    },
                    Err(_) => return,
                };
                match polled {
                    Ok(Some(status)) => break Ok(status),
                    Ok(None) => std::thread::sleep(std::time::Duration::from_millis(30)),
                    Err(e) => break Err(e),
                }
            };

            let err_buf = err_handle.join().unwrap_or_default();
            // From here on cancel() must not touch the output: the worker owns the outcome.
            finished_clone.store(true, Ordering::SeqCst);
            let is_cancelled = cancelled_clone.load(Ordering::SeqCst);

            if is_cancelled {
                let _ = std::fs::remove_file(&out_path_worker);
                if let Ok(mut res) = result_clone.lock() {
                    *res = Some(Err("Export cancelled".to_string()));
                }
            } else {
                match wait_status {
                    Ok(status) if status.success() => {
                        if let Ok(mut p) = progress_clone.lock() {
                            *p = 1.0;
                        }
                        if let Ok(mut res) = result_clone.lock() {
                            *res = Some(Ok(out_path_worker));
                        }
                    }
                    Ok(status) => {
                        let _ = std::fs::remove_file(&out_path_worker);
                        let err_str = String::from_utf8_lossy(&err_buf);
                        let summary = err_str
                            .lines()
                            .rev()
                            .map(str::trim)
                            .find(|l| !l.is_empty() && !l.starts_with("frame=") && !l.starts_with("fps="))
                            .unwrap_or("FFmpeg export failed");
                        if let Ok(mut res) = result_clone.lock() {
                            *res = Some(Err(if summary.contains("Encoder not found") || summary.contains("Unknown encoder") {
                                "This ffmpeg build has no H.264 encoder (libx264). Use Fast mode, or install a full ffmpeg build.".to_string()
                            } else {
                                format!("FFmpeg failed ({status}): {summary}")
                            }));
                        }
                    }
                    Err(e) => {
                        let _ = std::fs::remove_file(&out_path_worker);
                        if let Ok(mut res) = result_clone.lock() {
                            *res = Some(Err(format!("FFmpeg wait error: {e}")));
                        }
                    }
                }
            }
        });

        Ok(Self {
            output_path,
            child: child_arc,
            cancelled,
            finished,
            progress,
            result,
            worker: Some(worker),
        })
    }

    /// Polls the state of the export job without blocking.
    pub fn poll(&mut self) -> ClipExportPoll {
        let is_cancelled = self.cancelled.load(Ordering::SeqCst);
        let res_opt = self.result.lock().ok().and_then(|mut r| r.take());
        match res_opt {
            Some(Ok(path)) => ClipExportPoll::Success(path),
            Some(Err(err)) => {
                if is_cancelled {
                    ClipExportPoll::Cancelled
                } else {
                    ClipExportPoll::Failed(err)
                }
            }
            None => {
                if is_cancelled {
                    ClipExportPoll::Cancelled
                } else {
                    let prog = self.progress.lock().map(|p| *p).unwrap_or(0.0);
                    ClipExportPoll::Running { progress: prog }
                }
            }
        }
    }

    /// Cancels a running export: kills ffmpeg and removes the partial file. Does nothing once
    /// the export has finished, so a completed clip is never deleted.
    pub fn cancel(&mut self) {
        if self.finished.load(Ordering::SeqCst) {
            return;
        }
        if !self.cancelled.swap(true, Ordering::SeqCst) {
            if let Ok(mut guard) = self.child.lock() {
                if let Some(c) = guard.as_mut() {
                    let _ = c.kill();
                }
            }
            // The worker removes the partial file after ffmpeg exits (it can't be deleted on
            // Windows while ffmpeg still has it open).
        }
    }

    /// Returns current progress in range [0.0, 1.0].
    pub fn progress(&self) -> f32 {
        self.progress.lock().map(|p| *p).unwrap_or(0.0)
    }
}

impl Drop for ClipExportJob {
    fn drop(&mut self) {
        self.cancel();
        if let Some(w) = self.worker.take() {
            let _ = w.join();
        }
    }
}

/// Reveals `path` in the native desktop file manager (Explorer on Windows, Finder on macOS).
pub fn show_in_folder(path: &Path) {
    #[cfg(windows)]
    {
        let mut cmd = std::process::Command::new("explorer");
        cmd.arg(format!("/select,{}", path.display()));
        let _ = cmd.spawn();
    }
    #[cfg(target_os = "macos")]
    {
        let _ = std::process::Command::new("open").arg("-R").arg(path).spawn();
    }
    #[cfg(all(unix, not(target_os = "macos"), not(target_os = "android")))]
    {
        if let Some(parent) = path.parent() {
            let _ = std::process::Command::new("xdg-open").arg(parent).spawn();
        }
    }
}
