use super::create_hidden_command;
use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
use cpal::{SampleFormat, Stream, StreamConfig};
use parking_lot::Mutex;
use std::collections::VecDeque;
use std::io::Read;
use std::path::PathBuf;
use std::process::{Child, Stdio};
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::Arc;
use std::thread;

pub struct AudioEngine {
    _stream: Option<Stream>,
    pub state: Arc<AudioSharedState>,
    worker_handle: Option<thread::JoinHandle<()>>,
    stop_worker: Arc<AtomicBool>,
    current_child: Arc<Mutex<Option<Child>>>,
}

pub struct AudioSharedState {
    pub buffer: Mutex<VecDeque<f32>>,
    pub volume: Mutex<f32>,
    pub muted: AtomicBool,
    pub is_playing: AtomicBool,
    pub samples_played: AtomicU64,
    pub sample_rate: u32,
    pub channels: u16,
    pub seek_offset_ms: AtomicU64,
    pub speed: Mutex<f32>,
    pub visualizer_levels: Mutex<[f32; 16]>,
    pub delay_ms: AtomicU64,
    pub delay_sign: AtomicBool,
}

impl AudioEngine {
    pub fn new() -> Result<Self, String> {
        let host = cpal::default_host();
        let device = host
            .default_output_device()
            .ok_or_else(|| "No default audio output device found".to_string())?;

        let supported_config = device
            .default_output_config()
            .map_err(|e| format!("Failed to get default audio config: {e}"))?;

        let sample_rate = supported_config.sample_rate();
        let channels = 2u16;

        let config = StreamConfig {
            channels,
            sample_rate,
            buffer_size: cpal::BufferSize::Default,
        };

        let state = Arc::new(AudioSharedState {
            buffer: Mutex::new(VecDeque::with_capacity(sample_rate as usize * 4)),
            volume: Mutex::new(1.0),
            muted: AtomicBool::new(false),
            is_playing: AtomicBool::new(false),
            samples_played: AtomicU64::new(0),
            sample_rate,
            channels,
            seek_offset_ms: AtomicU64::new(0),
            speed: Mutex::new(1.0),
            visualizer_levels: Mutex::new([0.0; 16]),
            delay_ms: AtomicU64::new(0),
            delay_sign: AtomicBool::new(true),
        });

        let state_cb = Arc::clone(&state);
        let err_fn = |err| eprintln!("Audio stream error: {err}");

        let stream = match supported_config.sample_format() {
            SampleFormat::F32 => device.build_output_stream(
                config,
                move |data: &mut [f32], _: &cpal::OutputCallbackInfo| {
                    render_audio_f32(data, &state_cb);
                },
                err_fn,
                None,
            ),
            SampleFormat::I16 => device.build_output_stream(
                config,
                move |data: &mut [i16], _: &cpal::OutputCallbackInfo| {
                    render_audio_i16(data, &state_cb);
                },
                err_fn,
                None,
            ),
            _ => device.build_output_stream(
                config,
                move |data: &mut [f32], _: &cpal::OutputCallbackInfo| {
                    render_audio_f32(data, &state_cb);
                },
                err_fn,
                None,
            ),
        }
        .map_err(|e| format!("Failed to build audio output stream: {e}"))?;

        stream
            .play()
            .map_err(|e| format!("Failed to start audio stream: {e}"))?;

        Ok(Self {
            _stream: Some(stream),
            state,
            worker_handle: None,
            stop_worker: Arc::new(AtomicBool::new(false)),
            current_child: Arc::new(Mutex::new(None)),
        })
    }

    pub fn start_playback(
        &mut self,
        path: PathBuf,
        seek_secs: f64,
        speed: f32,
        track_index: Option<usize>,
    ) {
        self.stop();

        self.state.samples_played.store(0, Ordering::SeqCst);
        self.state
            .seek_offset_ms
            .store((seek_secs * 1000.0) as u64, Ordering::SeqCst);
        *self.state.speed.lock() = speed;

        let stop_flag = Arc::new(AtomicBool::new(false));
        self.stop_worker = Arc::clone(&stop_flag);
        let shared = Arc::clone(&self.state);
        let current_child = Arc::clone(&self.current_child);

        let sample_rate = self.state.sample_rate;
        let channels = self.state.channels;

        let handle = thread::spawn(move || {
            run_audio_decoder(
                path,
                seek_secs,
                speed,
                track_index,
                sample_rate,
                channels,
                shared,
                stop_flag,
                current_child,
            );
        });

        self.worker_handle = Some(handle);
        self.state.is_playing.store(true, Ordering::SeqCst);
    }

    pub fn pause(&self) {
        self.state.is_playing.store(false, Ordering::SeqCst);
    }

    pub fn resume(&self) {
        self.state.is_playing.store(true, Ordering::SeqCst);
    }

    pub fn is_playing(&self) -> bool {
        self.state.is_playing.load(Ordering::SeqCst)
    }

    pub fn set_volume(&self, vol: f32) {
        *self.state.volume.lock() = vol.clamp(0.0, 2.0);
    }

    pub fn get_volume(&self) -> f32 {
        *self.state.volume.lock()
    }

    pub fn set_muted(&self, muted: bool) {
        self.state.muted.store(muted, Ordering::SeqCst);
    }

    pub fn is_muted(&self) -> bool {
        self.state.muted.load(Ordering::SeqCst)
    }

    pub fn set_delay(&self, delay_ms: i64) {
        if delay_ms >= 0 {
            self.state.delay_sign.store(true, Ordering::SeqCst);
            self.state.delay_ms.store(delay_ms as u64, Ordering::SeqCst);
        } else {
            self.state.delay_sign.store(false, Ordering::SeqCst);
            self.state.delay_ms.store((-delay_ms) as u64, Ordering::SeqCst);
        }
    }

    pub fn get_delay(&self) -> i64 {
        let val = self.state.delay_ms.load(Ordering::SeqCst) as i64;
        if self.state.delay_sign.load(Ordering::SeqCst) {
            val
        } else {
            -val
        }
    }

    pub fn current_time_secs(&self) -> f64 {
        let offset_ms = self.state.seek_offset_ms.load(Ordering::SeqCst) as f64;
        let samples = self.state.samples_played.load(Ordering::SeqCst) as f64;
        let sr = self.state.sample_rate as f64;
        let ch = self.state.channels as f64;
        let speed = *self.state.speed.lock() as f64;

        let delay_ms = self.get_delay() as f64;

        let played_secs = (samples / (ch * sr)) * speed;
        (offset_ms / 1000.0) + played_secs + (delay_ms / 1000.0)
    }

    pub fn get_visualizer(&self) -> [f32; 16] {
        *self.state.visualizer_levels.lock()
    }

    pub fn stop(&mut self) {
        self.stop_worker.store(true, Ordering::SeqCst);
        if let Some(mut c) = self.current_child.lock().take() {
            let _ = c.kill();
            let _ = c.wait();
        }
        if let Some(h) = self.worker_handle.take() {
            let _ = h.join();
        }
        self.state.is_playing.store(false, Ordering::SeqCst);
        let mut buf = self.state.buffer.lock();
        buf.clear();
        self.state.samples_played.store(0, Ordering::SeqCst);
        self.state.seek_offset_ms.store(0, Ordering::SeqCst);
        let mut vis = self.state.visualizer_levels.lock();
        *vis = [0.0; 16];
    }
}

fn render_audio_f32(data: &mut [f32], state: &Arc<AudioSharedState>) {
    let is_playing = state.is_playing.load(Ordering::SeqCst);
    let is_muted = state.muted.load(Ordering::SeqCst);
    let volume = *state.volume.lock();

    if !is_playing {
        data.fill(0.0);
        return;
    }

    let mut buf = state.buffer.lock();
    let mut vis_bands = [0.0f32; 16];

    for (i, sample_out) in data.iter_mut().enumerate() {
        if let Some(sample) = buf.pop_front() {
            let processed = if is_muted { 0.0 } else { sample * volume };
            *sample_out = processed;

            let band_idx = i % 16;
            vis_bands[band_idx] = vis_bands[band_idx].max(processed.abs());
        } else {
            *sample_out = 0.0;
        }
    }

    state
        .samples_played
        .fetch_add(data.len() as u64, Ordering::SeqCst);

    let mut vis = state.visualizer_levels.lock();
    for i in 0..16 {
        vis[i] = vis[i] * 0.75 + vis_bands[i] * 0.25;
    }
}

fn render_audio_i16(data: &mut [i16], state: &Arc<AudioSharedState>) {
    let mut temp = vec![0.0f32; data.len()];
    render_audio_f32(&mut temp, state);
    for (out, val) in data.iter_mut().zip(temp.iter()) {
        *out = (val.clamp(-1.0, 1.0) * i16::MAX as f32) as i16;
    }
}

fn run_audio_decoder(
    path: PathBuf,
    seek_secs: f64,
    speed: f32,
    track_index: Option<usize>,
    sample_rate: u32,
    channels: u16,
    state: Arc<AudioSharedState>,
    stop_flag: Arc<AtomicBool>,
    current_child: Arc<Mutex<Option<Child>>>,
) {
    let mut cmd = create_hidden_command("ffmpeg");
    cmd.args(["-hide_banner", "-loglevel", "error"]);

    if seek_secs > 0.05 {
        cmd.args(["-ss", &format!("{seek_secs:.3}")]);
    }

    let path_str = path.to_string_lossy().to_string();
    cmd.args(["-i", &path_str]);

    if let Some(track) = track_index {
        cmd.args(["-map", &format!("0:a:{track}")]);
    } else {
        cmd.args(["-map", "0:a:0?"]);
    }

    if (speed - 1.0).abs() > 0.02 {
        cmd.args(["-filter:a", &format!("atempo={speed:.3}")]);
    }

    cmd.args([
        "-vn",
        "-sn",
        "-f",
        "f32le",
        "-ac",
        &channels.to_string(),
        "-ar",
        &sample_rate.to_string(),
        "-",
    ]);

    cmd.stdout(Stdio::piped());
    cmd.stderr(Stdio::null());

    let mut child: Child = match cmd.spawn() {
        Ok(c) => c,
        Err(e) => {
            eprintln!("Failed to spawn audio ffmpeg decoder: {e}");
            return;
        }
    };

    let mut stdout = match child.stdout.take() {
        Some(s) => s,
        None => return,
    };

    *current_child.lock() = Some(child);

    let mut chunk = [0u8; 8192];
    let max_buffer_samples = (sample_rate as usize * 4) * channels as usize;

    loop {
        if stop_flag.load(Ordering::SeqCst) {
            break;
        }

        {
            let current_len = state.buffer.lock().len();
            if current_len >= max_buffer_samples {
                thread::sleep(std::time::Duration::from_millis(20));
                continue;
            }
        }

        match stdout.read(&mut chunk) {
            Ok(0) => {
                break;
            }
            Ok(n) => {
                let num_samples = n / 4;
                let mut samples = Vec::with_capacity(num_samples);
                for i in 0..num_samples {
                    let bytes = [
                        chunk[i * 4],
                        chunk[i * 4 + 1],
                        chunk[i * 4 + 2],
                        chunk[i * 4 + 3],
                    ];
                    samples.push(f32::from_le_bytes(bytes));
                }

                let mut buf = state.buffer.lock();
                buf.extend(samples);
            }
            Err(e) => {
                eprintln!("Audio decode pipe read error: {e}");
                break;
            }
        }
    }

    if let Some(mut c) = current_child.lock().take() {
        let _ = c.kill();
        let _ = c.wait();
    }
}
