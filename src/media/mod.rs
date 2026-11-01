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

use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::Arc;

pub fn create_hidden_command(program: &str) -> Command {
    let resolved = resolve_binary_path(program);
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
        }
    }

    pub fn load_file<P: AsRef<Path>>(&mut self, path: P) -> Result<(), String> {
        let path_buf = path.as_ref().to_path_buf();
        let info = MediaInfo::probe(&path_buf)?;

        self.stop();

        self.current_file = Some(path_buf.clone());
        self.duration_secs = info.duration_seconds();
        self.media_info = Some(info.clone());
        self.manual_clock_secs = 0.0;
        self.last_tick = std::time::Instant::now();

        // Discover and extract embedded subtitle streams
        self.subtitles.clear();
        self.active_subtitle_track = None;

        for (idx, sub_stream) in info.subtitle_streams().iter().enumerate() {
            let lang = sub_stream
                .tags
                .as_ref()
                .and_then(|t| t.get("language"))
                .and_then(|v| v.as_str())
                .unwrap_or("Unknown")
                .to_string();
            let name = sub_stream
                .tags
                .as_ref()
                .and_then(|t| t.get("title"))
                .and_then(|v| v.as_str())
                .unwrap_or(&format!("Track {}", idx + 1))
                .to_string();

            if let Ok(track) =
                SubtitleTrack::from_embedded(&path_buf, sub_stream.index, name, lang)
            {
                self.subtitles.push(track);
            }
        }

        if !self.subtitles.is_empty() {
            self.active_subtitle_track = Some(0);
        }

        self.start_playback_at(0.0);

        Ok(())
    }

    pub fn start_playback_at(&mut self, seek_secs: f64) {
        let path = match &self.current_file {
            Some(p) => p.clone(),
            None => return,
        };

        self.manual_clock_secs = seek_secs;
        self.last_tick = std::time::Instant::now();

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
        if let Some(audio) = &self.audio_engine {
            audio.resume();
        }
        self.video_decoder.resume();
    }

    pub fn stop(&mut self) {
        self.is_playing = false;
        if let Some(audio) = &mut self.audio_engine {
            audio.stop();
        }
        self.video_decoder.stop();
        self.manual_clock_secs = 0.0;
    }

    pub fn seek(&mut self, target_secs: f64) {
        let clamped = target_secs.clamp(0.0, self.duration_secs.max(0.1));
        self.start_playback_at(clamped);
    }

    pub fn step_frame(&mut self, forward: bool) {
        let fps = self.media_info.as_ref().map_or(30.0, |m| m.fps());
        let frame_duration = 1.0 / fps;
        let cur = self.current_time_secs();
        let target = if forward {
            cur + frame_duration
        } else {
            (cur - frame_duration).max(0.0)
        };
        self.seek(target);
        self.pause();
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
        let track = SubtitleTrack::from_file(path)?;
        self.subtitles.push(track);
        self.active_subtitle_track = Some(self.subtitles.len() - 1);
        Ok(())
    }

    pub fn select_audio_track(&mut self, track_idx: usize) {
        self.active_audio_track = track_idx;
        let cur = self.current_time_secs();
        self.start_playback_at(cur);
    }

    pub fn select_subtitle_track(&mut self, track_idx: Option<usize>) {
        self.active_subtitle_track = track_idx;
    }

    pub fn current_time_secs(&mut self) -> f64 {
        let has_audio = self
            .media_info
            .as_ref()
            .map_or(false, |m| !m.audio_streams().is_empty());

        if let Some(audio) = &self.audio_engine {
            if has_audio {
                return audio.current_time_secs().min(self.duration_secs);
            }
        }

        if self.is_playing {
            let elapsed = self.last_tick.elapsed().as_secs_f64() * (self.playback_speed as f64);
            self.last_tick = std::time::Instant::now();
            self.manual_clock_secs += elapsed;
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

    pub fn take_snapshot(&self) -> Result<PathBuf, String> {
        let frame = self
            .video_decoder
            .get_latest_frame()
            .ok_or_else(|| "No video frame available to snapshot".to_string())?;

        use std::time::{SystemTime, UNIX_EPOCH};
        let epoch_secs = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs();

        let filename = format!("snapshot_{epoch_secs}.png");
        let path = PathBuf::from(&filename);

        image::save_buffer(
            &path,
            &frame.rgba_data,
            frame.width,
            frame.height,
            image::ExtendedColorType::Rgba8,
        )
        .map_err(|e| format!("Failed to save snapshot: {e}"))?;

        Ok(path)
    }

    pub fn take_new_video_frame(&self) -> Option<VideoFrame> {
        self.video_decoder.take_new_frame()
    }

    pub fn get_latest_video_frame(&self) -> Option<VideoFrame> {
        self.video_decoder.get_latest_frame()
    }
}
