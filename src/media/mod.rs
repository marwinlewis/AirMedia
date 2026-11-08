pub mod audio;
pub mod decoder;
pub mod dependency;
pub mod probe;
pub mod subtitle;

use audio::AudioEngine;
use decoder::{HwAccelMode, VideoDecoder, VideoFrame};
use probe::MediaInfo;
use subtitle::SubtitleTrack;

pub use dependency::{
    check_dependencies, locate_binary, resolve_binary_path, CodecDownloader, CodecLocation,
    DependencyStatus, DownloadState,
};

use crossbeam_channel::Receiver;
use std::io::Read;
use std::path::{Path, PathBuf};
use std::process::{Command, Output, Stdio};
use std::sync::Arc;
use std::time::{Duration, Instant};

/// Largest frame edge accepted from container metadata. Anything larger is treated as a
/// corrupt or malicious header instead of allocating a multi-gigabyte frame buffer.
pub const MAX_VIDEO_DIMENSION: u32 = 8192;

/// Protocols ffmpeg may use while reading a local file. Keeps crafted playlists/containers
/// (HLS, concat, ...) from reaching out to the network.
const LOCAL_PROTOCOLS: &str = "file,crypto,data";
/// Protocols allowed for user-entered network streams. `file` is deliberately excluded so a
/// remote manifest cannot point ffmpeg at local files.
const NETWORK_PROTOCOLS: &str = "http,https,tcp,tls,crypto,data,hls,rtmp,rtmps,rtsp,udp,rtp,srtp";
/// Network read timeout handed to ffmpeg, in microseconds.
const NETWORK_TIMEOUT_US: &str = "10000000";

/// Returns true when `input` looks like a `scheme://` URL rather than a filesystem path.
pub fn is_url(input: &Path) -> bool {
    let s = input.to_string_lossy();
    match s.find("://") {
        Some(i) if i >= 2 => s[..i]
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || c == '+' || c == '-' || c == '.'),
        _ => false,
    }
}

/// Input options for every ffmpeg/ffprobe invocation: a protocol whitelist, a network timeout
/// for streams, and an explicit `-i` so inputs starting with `-` are never parsed as options.
/// Local paths are made absolute so a relative name like `concat:a|b` stays a plain file.
pub fn ffmpeg_input_args(input: &Path) -> Vec<String> {
    let mut args = Vec::with_capacity(6);
    let input_str = if is_url(input) {
        args.extend([
            "-protocol_whitelist".to_string(),
            NETWORK_PROTOCOLS.to_string(),
            "-rw_timeout".to_string(),
            NETWORK_TIMEOUT_US.to_string(),
        ]);
        input.to_string_lossy().to_string()
    } else {
        args.extend(["-protocol_whitelist".to_string(), LOCAL_PROTOCOLS.to_string()]);
        std::path::absolute(input)
            .unwrap_or_else(|_| input.to_path_buf())
            .to_string_lossy()
            .to_string()
    };
    args.push("-i".to_string());
    args.push(input_str);
    args
}

/// Runs `cmd` capturing stdout, killing it if it has not exited within `timeout`.
pub fn output_with_timeout(mut cmd: Command, timeout: Duration) -> Result<Output, String> {
    cmd.stdout(Stdio::piped()).stderr(Stdio::piped()).stdin(Stdio::null());
    let mut child = cmd.spawn().map_err(|e| e.to_string())?;
    let mut stdout = child.stdout.take();
    let mut stderr = child.stderr.take();
    let out_reader = std::thread::spawn(move || {
        let mut buf = Vec::new();
        if let Some(s) = stdout.as_mut() {
            let _ = s.read_to_end(&mut buf);
        }
        buf
    });
    let err_reader = std::thread::spawn(move || {
        let mut buf = Vec::new();
        if let Some(s) = stderr.as_mut() {
            let _ = s.read_to_end(&mut buf);
        }
        buf
    });

    let deadline = Instant::now() + timeout;
    let status = loop {
        match child.try_wait() {
            Ok(Some(status)) => break status,
            Ok(None) if Instant::now() >= deadline => {
                let _ = child.kill();
                let _ = child.wait();
                return Err(format!("timed out after {}s", timeout.as_secs()));
            }
            Ok(None) => std::thread::sleep(Duration::from_millis(20)),
            Err(e) => return Err(e.to_string()),
        }
    };

    Ok(Output {
        status,
        stdout: out_reader.join().unwrap_or_default(),
        stderr: err_reader.join().unwrap_or_default(),
    })
}

/// Octave-band centre frequencies for the 10-band equalizer.
pub const EQ_BANDS_HZ: [u32; 10] = [31, 63, 125, 250, 500, 1000, 2000, 4000, 8000, 16000];

/// Builds an ffmpeg audio filter chain for the equalizer, or `None` when it would be a no-op.
pub fn equalizer_filter(preamp_db: f32, gains_db: &[f32; 10]) -> Option<String> {
    let mut parts = Vec::new();
    if preamp_db.abs() >= 0.05 {
        parts.push(format!("volume={preamp_db:.1}dB"));
    }
    for (hz, gain) in EQ_BANDS_HZ.iter().zip(gains_db) {
        if gain.abs() >= 0.05 {
            parts.push(format!("equalizer=f={hz}:t=o:w=1:g={gain:.1}"));
        }
    }
    if parts.is_empty() {
        None
    } else {
        Some(parts.join(","))
    }
}

/// Builds an ffmpeg `eq` video filter, or `None` when every value is at its default.
pub fn video_adjust_filter(brightness: f32, contrast: f32, saturation: f32, gamma: f32) -> Option<String> {
    let is_default = brightness.abs() < 0.005
        && (contrast - 1.0).abs() < 0.005
        && (saturation - 1.0).abs() < 0.005
        && (gamma - 1.0).abs() < 0.005;
    if is_default {
        None
    } else {
        Some(format!(
            "eq=brightness={brightness:.3}:contrast={contrast:.3}:saturation={saturation:.3}:gamma={gamma:.3}"
        ))
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, serde::Serialize, serde::Deserialize)]
pub enum ScreenshotFormat {
    #[default]
    Jpg,
    Png,
    Bmp,
}

impl ScreenshotFormat {
    pub fn extension(&self) -> &'static str {
        match self {
            Self::Jpg => "jpg",
            Self::Png => "png",
            Self::Bmp => "bmp",
        }
    }

    pub fn name(&self) -> &'static str {
        match self {
            Self::Jpg => "JPEG (.jpg)",
            Self::Png => "PNG (.png)",
            Self::Bmp => "BMP (.bmp)",
        }
    }

    pub const ALL: [ScreenshotFormat; 3] = [
        ScreenshotFormat::Jpg,
        ScreenshotFormat::Png,
        ScreenshotFormat::Bmp,
    ];
}

pub fn default_pictures_dir() -> PathBuf {
    #[cfg(windows)]
    {
        if let Ok(profile) = std::env::var("USERPROFILE") {
            let p = PathBuf::from(profile).join("Pictures");
            if p.exists() || std::fs::create_dir_all(&p).is_ok() {
                return p;
            }
        }
    }
    #[cfg(not(windows))]
    {
        if let Ok(home) = std::env::var("HOME") {
            let p = PathBuf::from(home).join("Pictures");
            if p.exists() || std::fs::create_dir_all(&p).is_ok() {
                return p;
            }
        }
    }
    std::env::current_dir().unwrap_or_else(|_| PathBuf::from("."))
}

pub fn create_hidden_command(program: &str) -> Command {
    let resolved = resolve_binary_path(program);
    #[allow(unused_mut)]
    let mut cmd = Command::new(resolved);
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        const CREATE_NO_WINDOW: u32 = 0x0800_0000;
        cmd.creation_flags(CREATE_NO_WINDOW);
    }
    cmd
}

pub struct MediaPlayer {
    pub current_file: Option<PathBuf>,
    pub media_info: Option<MediaInfo>,
    pub audio_engine: Option<AudioEngine>,
    pub video_decoder: VideoDecoder,
    pub subtitles: Vec<SubtitleTrack>,
    pub active_subtitle_track: Option<usize>,
    pub subtitle_delay_ms: i64,
    pub active_audio_track: usize,
    pub duration_secs: f64,
    pub playback_speed: f32,
    pub hw_accel: HwAccelMode,
    pub is_playing: bool,
    pub manual_clock_secs: f64,
    pub last_tick: std::time::Instant,
    pub stepped_while_paused: bool,
    /// ffmpeg audio filter chain (equalizer), applied on the next (re)start.
    pub audio_filter: Option<String>,
    /// ffmpeg video filter chain (color adjustments), applied on the next (re)start.
    pub video_filter: Option<String>,
    /// When the current pipeline was (re)started; used to detect media that fails instantly.
    pub playback_started_at: Instant,
    last_error: Option<String>,
    embedded_subs_rx: Option<Receiver<SubtitleTrack>>,
    auto_select_subtitle: bool,
}

impl MediaPlayer {
    pub fn new() -> Self {
        let audio_engine = match AudioEngine::new() {
            Ok(engine) => Some(engine),
            Err(e) => {
                eprintln!("Audio initialization warning: {e}");
                None
            }
        };

        Self {
            current_file: None,
            media_info: None,
            audio_engine,
            video_decoder: VideoDecoder::new(),
            subtitles: Vec::new(),
            active_subtitle_track: None,
            subtitle_delay_ms: 0,
            active_audio_track: 0,
            duration_secs: 0.0,
            playback_speed: 1.0,
            hw_accel: HwAccelMode::Auto,
            is_playing: false,
            manual_clock_secs: 0.0,
            last_tick: std::time::Instant::now(),
            stepped_while_paused: false,
            audio_filter: None,
            video_filter: None,
            playback_started_at: Instant::now(),
            last_error: None,
            embedded_subs_rx: None,
            auto_select_subtitle: false,
        }
    }

    /// Returns (and clears) the most recent load/playback error, for the UI to show.
    pub fn take_error(&mut self) -> Option<String> {
        self.last_error.take()
    }

    pub fn load_file<P: AsRef<Path>>(&mut self, path: P) -> Result<(), String> {
        let path_buf = path.as_ref().to_path_buf();
        let result = self.load_file_inner(path_buf.clone());
        if let Err(e) = &result {
            // Don't keep playing the previous file under the new playlist selection.
            self.unload();
            let name = path_buf
                .file_name()
                .map(|n| n.to_string_lossy().to_string())
                .unwrap_or_else(|| path_buf.to_string_lossy().to_string());
            self.last_error = Some(format!("Cannot open {name}: {e}"));
        }
        result
    }

    fn load_file_inner(&mut self, path_buf: PathBuf) -> Result<(), String> {
        let info = MediaInfo::probe(&path_buf)?;
        info.validate()?;

        self.stop();

        self.current_file = Some(path_buf.clone());
        self.duration_secs = info.duration_seconds();
        self.media_info = Some(info.clone());
        self.manual_clock_secs = 0.0;
        self.last_tick = std::time::Instant::now();
        self.stepped_while_paused = false;

        self.subtitles.clear();
        self.active_subtitle_track = None;
        self.auto_select_subtitle = true;

        // Sidecar files next to the media (movie.srt, movie.en.srt, ...) load instantly.
        for sidecar in subtitle::find_sidecar_subtitles(&path_buf) {
            if let Ok(track) = SubtitleTrack::from_file(&sidecar) {
                self.subtitles.push(track);
            }
        }
        if !self.subtitles.is_empty() {
            self.active_subtitle_track = Some(0);
            self.auto_select_subtitle = false;
        }

        // Embedded text tracks need a full demux pass, so extract them off the UI thread and
        // attach them as they arrive (see `poll_background`).
        let embedded: Vec<(usize, String, String)> = info
            .subtitle_streams()
            .iter()
            .enumerate()
            .filter(|(_, s)| !subtitle::is_bitmap_subtitle_codec(s.codec_name.as_deref()))
            .map(|(idx, s)| {
                let tag = |key: &str| s.tags.as_ref().and_then(|t| t.get(key)).and_then(|v| v.as_str());
                let lang = tag("language").unwrap_or("Unknown").to_string();
                let name = tag("title").map(str::to_string).unwrap_or_else(|| format!("Track {}", idx + 1));
                (s.index, name, lang)
            })
            .collect();
        self.embedded_subs_rx = None;
        if !embedded.is_empty() {
            let (tx, rx) = crossbeam_channel::unbounded();
            let media = path_buf.clone();
            std::thread::spawn(move || {
                for (stream_index, name, lang) in embedded {
                    if let Ok(track) = SubtitleTrack::from_embedded(&media, stream_index, name, lang) {
                        if tx.send(track).is_err() {
                            break; // another file was loaded meanwhile
                        }
                    }
                }
            });
            self.embedded_subs_rx = Some(rx);
        }

        self.start_playback_at(0.0);

        Ok(())
    }

    /// Attaches results from background work (embedded subtitle extraction). Call once per frame.
    pub fn poll_background(&mut self) {
        let Some(rx) = &self.embedded_subs_rx else { return };
        let mut disconnected = false;
        loop {
            match rx.try_recv() {
                Ok(track) => {
                    self.subtitles.push(track);
                    if self.auto_select_subtitle && self.active_subtitle_track.is_none() {
                        self.active_subtitle_track = Some(self.subtitles.len() - 1);
                    }
                    self.auto_select_subtitle = false;
                }
                Err(crossbeam_channel::TryRecvError::Empty) => break,
                Err(crossbeam_channel::TryRecvError::Disconnected) => {
                    disconnected = true;
                    break;
                }
            }
        }
        if disconnected {
            self.embedded_subs_rx = None;
        }
    }

    fn has_audio_stream(&self) -> bool {
        self.media_info.as_ref().is_some_and(|m| !m.audio_streams().is_empty())
    }

    /// True once every active decode pipeline has reached end of stream and the audio buffer
    /// has drained, i.e. the current item has played to the end.
    pub fn is_finished(&self) -> bool {
        if self.current_file.is_none() {
            return false;
        }
        let video_active = self.video_decoder.has_worker();
        let audio_active = self.has_audio_stream() && self.audio_engine.is_some();
        if !video_active && !audio_active {
            return self.duration_secs > 0.5 && self.manual_clock_secs >= self.duration_secs - 0.05;
        }
        let video_done = !video_active || self.video_decoder.is_finished();
        let audio_done = !audio_active || self.audio_engine.as_ref().is_some_and(|a| a.is_drained());
        video_done && audio_done
    }

    /// The decoder actually in use, which differs from `hw_accel` after a software fallback.
    pub fn effective_hw_accel(&self) -> HwAccelMode {
        if self.video_decoder.fell_back_to_software() {
            HwAccelMode::Software
        } else {
            self.hw_accel
        }
    }

    /// Sets the equalizer filter chain and restarts playback at the current position.
    pub fn set_audio_filter(&mut self, filter: Option<String>) {
        if self.audio_filter == filter {
            return;
        }
        self.audio_filter = filter;
        self.restart_in_place();
    }

    /// Sets the color-adjustment filter chain and restarts playback at the current position.
    pub fn set_video_filter(&mut self, filter: Option<String>) {
        if self.video_filter == filter {
            return;
        }
        self.video_filter = filter;
        self.restart_in_place();
    }

    fn restart_in_place(&mut self) {
        if self.current_file.is_none() {
            return;
        }
        let was_playing = self.is_playing;
        let cur = self.current_time_secs();
        self.start_playback_at(cur);
        if !was_playing {
            self.pause();
        }
    }

    pub fn start_playback_at(&mut self, seek_secs: f64) {
        let path = match &self.current_file {
            Some(p) => p.clone(),
            None => return,
        };

        self.manual_clock_secs = seek_secs;
        self.last_tick = std::time::Instant::now();
        self.playback_started_at = Instant::now();

        let has_audio = self
            .media_info
            .as_ref()
            .map_or(false, |m| !m.audio_streams().is_empty());

        if let Some(audio) = &mut self.audio_engine {
            if has_audio {
                audio.start_playback(
                    path.clone(),
                    seek_secs,
                    self.playback_speed,
                    Some(self.active_audio_track),
                    self.audio_filter.clone(),
                );
            } else {
                audio.stop();
            }
        }

        if let Some(info) = &self.media_info {
            if let Some((w, h)) = info.video_dimensions() {
                let fps = info.fps();

                let audio_clock_fn: Option<Arc<dyn Fn() -> f64 + Send + Sync>> =
                    if let Some(ref audio) = self.audio_engine {
                        if has_audio {
                            let state_clone = Arc::clone(&audio.state);
                            Some(Arc::new(move || {
                                let offset_ms =
                                    state_clone.seek_offset_ms.load(std::sync::atomic::Ordering::SeqCst) as f64;
                                let samples =
                                    state_clone.samples_played.load(std::sync::atomic::Ordering::SeqCst) as f64;
                                let sr = state_clone.sample_rate as f64;
                                let ch = state_clone.channels as f64;
                                let sp = *state_clone.speed.lock() as f64;
                                (offset_ms / 1000.0) + (samples / (ch * sr)) * sp
                            }))
                        } else {
                            None
                        }
                    } else {
                        None
                    };

                self.video_decoder.start(
                    path,
                    w,
                    h,
                    fps,
                    seek_secs,
                    self.playback_speed,
                    self.hw_accel,
                    self.video_filter.clone(),
                    audio_clock_fn,
                );
            }
        }

        self.is_playing = true;
    }

    pub fn toggle_play(&mut self) {
        if self.is_playing {
            self.pause();
        } else {
            self.resume();
        }
    }

    pub fn pause(&mut self) {
        self.is_playing = false;
        if let Some(audio) = &self.audio_engine {
            audio.pause();
        }
        self.video_decoder.pause();
    }

    pub fn resume(&mut self) {
        self.is_playing = true;
        self.last_tick = std::time::Instant::now();
        if self.stepped_while_paused {
            self.stepped_while_paused = false;
            let cur = self.current_time_secs();
            self.start_playback_at(cur);
            return;
        }
        if let Some(audio) = &self.audio_engine {
            audio.resume();
        }
        self.video_decoder.resume();
    }

    pub fn stop(&mut self) {
        self.is_playing = false;
        self.stepped_while_paused = false;
        if let Some(audio) = &mut self.audio_engine {
            audio.stop();
        }
        self.video_decoder.stop();
        self.manual_clock_secs = 0.0;
    }

    /// Stops playback and forgets the current file, leaving the player idle.
    pub fn unload(&mut self) {
        self.stop();
        self.current_file = None;
        self.media_info = None;
        self.duration_secs = 0.0;
        self.subtitles.clear();
        self.active_subtitle_track = None;
        self.embedded_subs_rx = None;
    }

    pub fn seek(&mut self, target_secs: f64) {
        let clamped = target_secs.clamp(0.0, self.duration_secs.max(0.1));
        self.stepped_while_paused = false;
        self.start_playback_at(clamped);
    }

    pub fn step_frame(&mut self, forward: bool) {
        if self.is_playing {
            self.pause();
        }
        self.stepped_while_paused = true;

        let fps = self.media_info.as_ref().map_or(30.0, |m| m.fps());
        let frame_duration = 1.0 / fps;

        if forward {
            if self.video_decoder.is_running() {
                self.video_decoder.step_forward();
                self.manual_clock_secs = (self.manual_clock_secs + frame_duration).min(self.duration_secs);
            } else {
                let cur = self.current_time_secs();
                let target = (cur + frame_duration).min(self.duration_secs);
                self.start_playback_at(target);
                self.pause();
            }
        } else {
            let cur = self.current_time_secs();
            let target = (cur - frame_duration).max(0.0);
            self.start_playback_at(target);
            self.pause();
        }
    }

    pub fn set_speed(&mut self, speed: f32) {
        let speed = speed.clamp(0.25, 3.0);
        self.playback_speed = speed;
        let cur = self.current_time_secs();
        self.start_playback_at(cur);
    }

    pub fn set_hw_accel(&mut self, mode: HwAccelMode) {
        self.hw_accel = mode;
        let cur = self.current_time_secs();
        self.start_playback_at(cur);
    }

    pub fn set_volume(&mut self, vol: f32) {
        if let Some(audio) = &self.audio_engine {
            audio.set_volume(vol);
        }
    }

    pub fn get_volume(&self) -> f32 {
        self.audio_engine
            .as_ref()
            .map_or(1.0, |a| a.get_volume())
    }

    pub fn toggle_mute(&mut self) {
        if let Some(audio) = &self.audio_engine {
            audio.set_muted(!audio.is_muted());
        }
    }

    pub fn is_muted(&self) -> bool {
        self.audio_engine.as_ref().map_or(false, |a| a.is_muted())
    }

    pub fn set_audio_delay(&mut self, delay_ms: i64) {
        if let Some(audio) = &self.audio_engine {
            audio.set_delay(delay_ms);
        }
    }

    pub fn get_audio_delay(&self) -> i64 {
        self.audio_engine.as_ref().map_or(0, |a| a.get_delay())
    }

    pub fn set_subtitle_delay(&mut self, delay_ms: i64) {
        self.subtitle_delay_ms = delay_ms;
    }

    pub fn get_subtitle_delay(&self) -> i64 {
        self.subtitle_delay_ms
    }

    pub fn load_external_subtitle<P: AsRef<Path>>(&mut self, path: P) -> Result<(), String> {
        let track = SubtitleTrack::from_file(path).inspect_err(|e| self.last_error = Some(e.clone()))?;
        self.subtitles.push(track);
        self.active_subtitle_track = Some(self.subtitles.len() - 1);
        self.auto_select_subtitle = false;
        Ok(())
    }

    pub fn select_audio_track(&mut self, track_idx: usize) {
        self.active_audio_track = track_idx;
        let cur = self.current_time_secs();
        self.start_playback_at(cur);
    }

    pub fn select_subtitle_track(&mut self, track_idx: Option<usize>) {
        self.active_subtitle_track = track_idx;
        self.auto_select_subtitle = false;
    }

    pub fn current_time_secs(&mut self) -> f64 {
        let has_audio = self
            .media_info
            .as_ref()
            .map_or(false, |m| !m.audio_streams().is_empty());

        if self.is_playing {
            if let Some(audio) = &self.audio_engine {
                if has_audio {
                    return audio.current_time_secs().min(self.duration_secs);
                }
            }

            let elapsed = self.last_tick.elapsed().as_secs_f64() * (self.playback_speed as f64);
            self.last_tick = std::time::Instant::now();
            self.manual_clock_secs += elapsed;
        } else if let Some(frame) = self.video_decoder.get_latest_frame() {
            self.manual_clock_secs = frame.pts_secs;
        }

        self.manual_clock_secs.min(self.duration_secs)
    }

    pub fn get_active_subtitle_text(&mut self) -> Option<String> {
        let cur_time = self.current_time_secs();
        let track_idx = self.active_subtitle_track?;
        let track = self.subtitles.get(track_idx)?;
        let cur_ms = (cur_time * 1000.0) as i64 + self.subtitle_delay_ms;
        track.get_text_at(cur_ms)
    }

    pub fn get_visualizer(&self) -> [f32; 16] {
        self.audio_engine
            .as_ref()
            .map_or([0.0; 16], |a| a.get_visualizer())
    }

    pub fn take_screenshot(
        &self,
        output_dir: &Path,
        format: ScreenshotFormat,
    ) -> Result<PathBuf, String> {
        let frame = self
            .video_decoder
            .get_latest_frame()
            .ok_or_else(|| "No video frame available to screenshot".to_string())?;

        if !output_dir.exists() {
            std::fs::create_dir_all(output_dir)
                .map_err(|e| format!("Failed to create folder {}: {e}", output_dir.display()))?;
        }

        let prefix = self
            .current_file
            .as_ref()
            .and_then(|p| p.file_stem())
            .and_then(|s| s.to_str())
            .unwrap_or("snapshot");

        let safe_prefix: String = prefix
            .chars()
            .map(|c| if c.is_alphanumeric() || c == '-' || c == '_' { c } else { '_' })
            .collect();

        use std::time::{SystemTime, UNIX_EPOCH};
        let epoch_millis = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_millis();

        let filename = format!("{safe_prefix}_{:.2}s_{epoch_millis}.{}", frame.pts_secs, format.extension());
        let path = output_dir.join(&filename);

        match format {
            ScreenshotFormat::Jpg => {
                let rgb_data: Vec<u8> = frame
                    .rgba_data
                    .chunks_exact(4)
                    .flat_map(|c| [c[0], c[1], c[2]])
                    .collect();
                image::save_buffer_with_format(
                    &path,
                    &rgb_data,
                    frame.width,
                    frame.height,
                    image::ExtendedColorType::Rgb8,
                    image::ImageFormat::Jpeg,
                )
                .map_err(|e| format!("Failed to save JPG screenshot: {e}"))?;
            }
            ScreenshotFormat::Png => {
                image::save_buffer_with_format(
                    &path,
                    &frame.rgba_data,
                    frame.width,
                    frame.height,
                    image::ExtendedColorType::Rgba8,
                    image::ImageFormat::Png,
                )
                .map_err(|e| format!("Failed to save PNG screenshot: {e}"))?;
            }
            ScreenshotFormat::Bmp => {
                let rgb_data: Vec<u8> = frame
                    .rgba_data
                    .chunks_exact(4)
                    .flat_map(|c| [c[0], c[1], c[2]])
                    .collect();
                image::save_buffer_with_format(
                    &path,
                    &rgb_data,
                    frame.width,
                    frame.height,
                    image::ExtendedColorType::Rgb8,
                    image::ImageFormat::Bmp,
                )
                .map_err(|e| format!("Failed to save BMP screenshot: {e}"))?;
            }
        }

        Ok(path)
    }

    pub fn take_snapshot(&self) -> Result<PathBuf, String> {
        self.take_screenshot(&default_pictures_dir(), ScreenshotFormat::Jpg)
    }

    pub fn take_new_video_frame(&self) -> Option<VideoFrame> {
        self.video_decoder.take_new_frame()
    }

    pub fn get_latest_video_frame(&self) -> Option<VideoFrame> {
        self.video_decoder.get_latest_frame()
    }
}
