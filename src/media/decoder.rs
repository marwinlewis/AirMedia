use super::create_hidden_command;
use parking_lot::Mutex;
use std::io::Read;
use std::path::{Path, PathBuf};
use std::process::{Child, Stdio};
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::Arc;
use std::thread;
use std::time::{Duration, Instant};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum HwAccelMode {
    #[default]
    Auto,
    D3D11VA,
    D3D12VA,
    Cuda,
    Vulkan,
    DXVA2,
    Software,
}

impl HwAccelMode {
    pub fn name(&self) -> &'static str {
        match self {
            Self::Auto => "Auto (GPU)",
            Self::D3D11VA => "DirectX 11 (D3D11VA)",
            Self::D3D12VA => "DirectX 12 (D3D12VA)",
            Self::Cuda => "NVIDIA NVDEC (CUDA)",
            Self::Vulkan => "Vulkan Video",
            Self::DXVA2 => "DirectX (DXVA2)",
            Self::Software => "Software (CPU)",
        }
    }

    pub fn short_name(&self) -> &'static str {
        match self {
            Self::Auto => "Auto",
            Self::D3D11VA => "D3D11",
            Self::D3D12VA => "D3D12",
            Self::Cuda => "NVDEC",
            Self::Vulkan => "Vulkan",
            Self::DXVA2 => "DXVA2",
            Self::Software => "CPU",
        }
    }

    pub fn ffmpeg_arg(&self) -> Option<&'static str> {
        match self {
            Self::Auto => Some("auto"),
            Self::D3D11VA => Some("d3d11va"),
            Self::D3D12VA => Some("d3d12va"),
            Self::Cuda => Some("cuda"),
            Self::Vulkan => Some("vulkan"),
            Self::DXVA2 => Some("dxva2"),
            Self::Software => None,
        }
    }

    pub const ALL: [HwAccelMode; 7] = [
        HwAccelMode::Auto,
        HwAccelMode::D3D11VA,
        HwAccelMode::D3D12VA,
        HwAccelMode::Cuda,
        HwAccelMode::Vulkan,
        HwAccelMode::DXVA2,
        HwAccelMode::Software,
    ];
}

#[derive(Clone)]
pub struct VideoFrame {
    pub width: u32,
    pub height: u32,
    pub pts_secs: f64,
    pub rgba_data: Arc<Vec<u8>>,
}

pub struct VideoDecoder {
    shared_frame: Arc<Mutex<Option<VideoFrame>>>,
    frame_ready: Arc<AtomicBool>,
    stop_flag: Arc<AtomicBool>,
    worker_handle: Option<thread::JoinHandle<()>>,
    is_paused: Arc<AtomicBool>,
    seek_target_ms: Arc<AtomicU64>,
    seek_requested: Arc<AtomicBool>,
    current_hw_accel: HwAccelMode,
}

impl VideoDecoder {
    pub fn new() -> Self {
        Self {
            shared_frame: Arc::new(Mutex::new(None)),
            frame_ready: Arc::new(AtomicBool::new(false)),
            stop_flag: Arc::new(AtomicBool::new(false)),
            worker_handle: None,
            is_paused: Arc::new(AtomicBool::new(false)),
            seek_target_ms: Arc::new(AtomicU64::new(0)),
            seek_requested: Arc::new(AtomicBool::new(false)),
            current_hw_accel: HwAccelMode::Auto,
        }
    }

    pub fn start(
        &mut self,
        path: PathBuf,
        width: u32,
        height: u32,
        fps: f64,
        seek_secs: f64,
        speed: f32,
        hw_accel: HwAccelMode,
        audio_clock_fn: Option<Arc<dyn Fn() -> f64 + Send + Sync>>,
    ) {
        self.stop();

        self.current_hw_accel = hw_accel;
        self.stop_flag = Arc::new(AtomicBool::new(false));
        self.is_paused = Arc::new(AtomicBool::new(false));
        self.frame_ready.store(false, Ordering::SeqCst);

        let shared_frame = Arc::clone(&self.shared_frame);
        let frame_ready = Arc::clone(&self.frame_ready);
        let stop_flag = Arc::clone(&self.stop_flag);
        let is_paused = Arc::clone(&self.is_paused);
        let seek_target_ms = Arc::clone(&self.seek_target_ms);
        let seek_requested = Arc::clone(&self.seek_requested);

        let handle = thread::spawn(move || {
            run_video_worker(
                path,
                width,
                height,
                fps,
                seek_secs,
                speed,
                hw_accel,
                shared_frame,
                frame_ready,
                stop_flag,
                is_paused,
                seek_target_ms,
                seek_requested,
                audio_clock_fn,
            );
        });

        self.worker_handle = Some(handle);
    }

    pub fn take_new_frame(&self) -> Option<VideoFrame> {
        if self.frame_ready.swap(false, Ordering::SeqCst) {
            let guard = self.shared_frame.lock();
            guard.clone()
        } else {
            None
        }
    }

    pub fn get_latest_frame(&self) -> Option<VideoFrame> {
        let guard = self.shared_frame.lock();
        guard.clone()
    }

    pub fn pause(&self) {
        self.is_paused.store(true, Ordering::SeqCst);
    }

    pub fn resume(&self) {
        self.is_paused.store(false, Ordering::SeqCst);
    }

    pub fn is_paused(&self) -> bool {
        self.is_paused.load(Ordering::SeqCst)
    }

    pub fn request_seek(&self, secs: f64) {
        self.seek_target_ms.store((secs * 1000.0) as u64, Ordering::SeqCst);
        self.seek_requested.store(true, Ordering::SeqCst);
    }

    pub fn current_hw_accel(&self) -> HwAccelMode {
        self.current_hw_accel
    }

    pub fn stop(&mut self) {
        self.stop_flag.store(true, Ordering::SeqCst);
        if let Some(h) = self.worker_handle.take() {
            let _ = h.join();
        }
        let mut frame = self.shared_frame.lock();
        *frame = None;
        self.frame_ready.store(false, Ordering::SeqCst);
    }
}

fn spawn_ffmpeg_decoder(
    path: &Path,
    seek_secs: f64,
    hw_accel: HwAccelMode,
) -> Result<Child, String> {
    let mut cmd = create_hidden_command("ffmpeg");
    cmd.args(["-hide_banner", "-loglevel", "error"]);

    if let Some(accel) = hw_accel.ffmpeg_arg() {
        cmd.args(["-hwaccel", accel]);
    }

    if seek_secs > 0.05 {
        cmd.args(["-ss", &format!("{seek_secs:.3}")]);
    }

    let path_str = path.to_string_lossy().to_string();
    cmd.args([
        "-i",
        &path_str,
        "-an",
        "-sn",
        "-f",
        "rawvideo",
        "-pix_fmt",
        "rgba",
        "-",
    ]);

    cmd.stdout(Stdio::piped());
    cmd.stderr(Stdio::null());

    cmd.spawn().map_err(|e| format!("Failed to spawn ffmpeg video process: {e}"))
}

fn run_video_worker(
    path: PathBuf,
    width: u32,
    height: u32,
    fps: f64,
    mut current_seek: f64,
    speed: f32,
    hw_accel: HwAccelMode,
    shared_frame: Arc<Mutex<Option<VideoFrame>>>,
    frame_ready: Arc<AtomicBool>,
    stop_flag: Arc<AtomicBool>,
    is_paused: Arc<AtomicBool>,
    seek_target_ms: Arc<AtomicU64>,
    seek_requested: Arc<AtomicBool>,
    audio_clock_fn: Option<Arc<dyn Fn() -> f64 + Send + Sync>>,
) {
    let frame_bytes = (width as usize) * (height as usize) * 4;
    if frame_bytes == 0 {
        return;
    }

    let frame_duration_secs = if fps > 0.0 { 1.0 / fps } else { 1.0 / 30.0 };

    'restart_loop: loop {
        if stop_flag.load(Ordering::SeqCst) {
            break;
        }

        let mut child = match spawn_ffmpeg_decoder(&path, current_seek, hw_accel) {
            Ok(c) => c,
            Err(e) => {
                // If specific hardware acceleration failed, fallback to software
                if hw_accel != HwAccelMode::Software {
                    eprintln!("HW Accel {hw_accel:?} failed ({e}), falling back to software decode");
                    match spawn_ffmpeg_decoder(&path, current_seek, HwAccelMode::Software) {
                        Ok(c) => c,
                        Err(e2) => {
                            eprintln!("Software fallback also failed: {e2}");
                            break;
                        }
                    }
                } else {
                    eprintln!("Video spawn error: {e}");
                    break;
                }
            }
        };

        let mut stdout = match child.stdout.take() {
            Some(s) => s,
            None => {
                let _ = child.kill();
                break;
            }
        };

        let mut frame_index = 0u64;
        let mut wall_clock_start = Instant::now();
        let wall_clock_seek_base = current_seek;

        loop {
            if stop_flag.load(Ordering::SeqCst) {
                let _ = child.kill();
                break 'restart_loop;
            }

            // Check if seek was requested
            if seek_requested.swap(false, Ordering::SeqCst) {
                let target_ms = seek_target_ms.load(Ordering::SeqCst);
                current_seek = (target_ms as f64) / 1000.0;
                let _ = child.kill();
                let _ = child.wait();
                continue 'restart_loop;
            }

            // Pause handling
            if is_paused.load(Ordering::SeqCst) {
                thread::sleep(Duration::from_millis(25));
                wall_clock_start = Instant::now();
                continue;
            }

            // Read exact frame buffer
            let mut buf = vec![0u8; frame_bytes];
            let mut read_bytes = 0;
            while read_bytes < frame_bytes {
                match stdout.read(&mut buf[read_bytes..]) {
                    Ok(0) => {
                        // End of stream
                        let _ = child.wait();
                        break 'restart_loop;
                    }
                    Ok(n) => {
                        read_bytes += n;
                    }
                    Err(e) => {
                        eprintln!("Video pipe read error: {e}");
                        let _ = child.kill();
                        break 'restart_loop;
                    }
                }
            }

            let frame_pts = current_seek + (frame_index as f64) * frame_duration_secs;
            frame_index += 1;

            // AV Sync Logic
            let master_time = if let Some(ref audio_clock) = audio_clock_fn {
                audio_clock()
            } else {
                wall_clock_seek_base + (wall_clock_start.elapsed().as_secs_f64() * speed as f64)
            };

            let delta = frame_pts - master_time;

            if delta > 0.002 {
                // Video is ahead of master clock -> wait
                let sleep_ms = (delta * 1000.0).clamp(1.0, 100.0);
                thread::sleep(Duration::from_millis(sleep_ms as u64));
            } else if delta < -0.060 {
                // Video is lagging behind master clock by >60ms -> skip frame display to catch up
                continue;
            }

            // Deliver frame to UI
            {
                let mut guard = shared_frame.lock();
                *guard = Some(VideoFrame {
                    width,
                    height,
                    pts_secs: frame_pts,
                    rgba_data: Arc::new(buf),
                });
            }
            frame_ready.store(true, Ordering::SeqCst);
        }
    }
}
