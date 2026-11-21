//! Clip trimming and export argument generation.

use std::path::Path;

/// Minimum clip duration in seconds. A clip shorter than this is rejected or clamped.
pub const MIN_CLIP_SECS: f64 = 0.1;

/// A bounded time range in seconds [start, end].
#[derive(Debug, Clone, Copy, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct ClipRange {
    pub start: f64,
    pub end: f64,
}

impl Default for ClipRange {
    fn default() -> Self {
        Self {
            start: 0.0,
            end: 10.0,
        }
    }
}

impl ClipRange {
    /// Creates a clamped `ClipRange` ensuring `start >= 0`, `start + MIN_CLIP_SECS <= end`,
    /// and `end <= total_duration`.
    pub fn new(start: f64, end: f64, total_duration: f64) -> Self {
        let dur = total_duration.max(0.0);
        let s = start.clamp(0.0, (dur - MIN_CLIP_SECS).max(0.0));
        let e = end.clamp(s + MIN_CLIP_SECS, dur);
        Self {
            start: s,
            end: e.max(s + MIN_CLIP_SECS.min(dur)),
        }
    }

    /// Creates a default 10-second clip starting at `start` (clamped to `total_duration`).
    pub fn default_for(start: f64, total_duration: f64) -> Self {
        let dur = total_duration.max(0.0);
        let s = start.clamp(0.0, (dur - MIN_CLIP_SECS).max(0.0));
        let e = (s + 10.0).min(dur);
        Self {
            start: s,
            end: e.max(s + MIN_CLIP_SECS.min(dur)),
        }
    }

    /// Returns the duration of the clip range in seconds.
    pub fn duration(&self) -> f64 {
        (self.end - self.start).max(0.0)
    }

    /// Clamps both start and end to `total_duration`, maintaining the minimum length.
    pub fn clamp(&mut self, total_duration: f64) {
        let dur = total_duration.max(0.0);
        self.start = self.start.clamp(0.0, (dur - MIN_CLIP_SECS).max(0.0));
        self.end = self.end.clamp(self.start + MIN_CLIP_SECS.min(dur), dur);
        if self.end < self.start + MIN_CLIP_SECS {
            self.end = (self.start + MIN_CLIP_SECS).min(dur);
        }
    }

    /// Sets the start time in seconds. If the existing end is before `start + MIN_CLIP_SECS`,
    /// end is pushed forward.
    pub fn set_start(&mut self, t: f64, total_duration: f64) {
        let dur = total_duration.max(0.0);
        let s = t.clamp(0.0, (dur - MIN_CLIP_SECS).max(0.0));
        self.start = s;
        if self.end < self.start + MIN_CLIP_SECS {
            self.end = (self.start + MIN_CLIP_SECS).min(dur);
        }
    }

    /// Sets the end time in seconds. Returns `false` if `t < self.start + MIN_CLIP_SECS`.
    pub fn set_end(&mut self, t: f64, total_duration: f64) -> bool {
        let dur = total_duration.max(0.0);
        if t < self.start + MIN_CLIP_SECS {
            return false;
        }
        self.end = t.min(dur);
        true
    }

    /// For range looping (Preview): when playback has passed `end` or is before `start`,
    /// returns the seek target (`start`).
    pub fn jump_target(&self, t: f64) -> Option<f64> {
        if t >= self.end || t < self.start {
            Some(self.start)
        } else {
            None
        }
    }
}

/// Encoding mode for clip exports.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, serde::Serialize, serde::Deserialize)]
pub enum ClipExportMode {
    #[default]
    Fast,
    Precise,
}

impl ClipExportMode {
    pub const ALL: [ClipExportMode; 2] = [ClipExportMode::Fast, ClipExportMode::Precise];

    pub fn name(&self) -> &'static str {
        match self {
            Self::Fast => "Fast — no re-encode, cuts at nearest keyframe",
            Self::Precise => "Precise — re-encode, frame-accurate",
        }
    }

    pub fn short_name(&self) -> &'static str {
        match self {
            Self::Fast => "Fast (Stream copy)",
            Self::Precise => "Precise (Re-encode)",
        }
    }
}

/// Encoder arguments for a Precise (re-encoded) export: H.264 + AAC, the most widely
/// available encoders in ffmpeg builds and accepted by every container in
/// [`PRECISE_EXTENSIONS`]. (VP9 for WebM is left out: many builds lack libvpx.)
pub const PRECISE_VIDEO_ARGS: [&str; 8] = ["-c:v", "libx264", "-crf", "18", "-preset", "veryfast", "-pix_fmt", "yuv420p"];
pub const PRECISE_AUDIO_ARGS: [&str; 4] = ["-c:a", "aac", "-b:a", "192k"];

/// Containers a Precise export can write. Anything else (e.g. `.webm`, `.ogv`, `.wmv`) is
/// exported with an `.mp4` extension instead.
pub const PRECISE_EXTENSIONS: [&str; 7] = ["mp4", "m4v", "mkv", "mov", "ts", "flv", "avi"];

/// Checks a user-entered clip filename and returns the full output path.
///
/// Rejects empty names, names containing path separators or `..`, names without an extension,
/// and any path that resolves to the source file itself (exporting over the source would
/// destroy it).
pub fn validate_clip_output(input: &Path, folder: &Path, filename: &str) -> Result<std::path::PathBuf, String> {
    let name = filename.trim();
    if name.is_empty() {
        return Err("Enter a file name".to_string());
    }
    if name.contains(['/', '\\']) || name == "." || name == ".." || name.contains("..") {
        return Err("The file name can't contain folders or \"..\"".to_string());
    }
    #[cfg(windows)]
    if name.contains([':', '*', '?', '"', '<', '>', '|']) {
        return Err("The file name contains characters Windows doesn't allow".to_string());
    }
    if Path::new(name).extension().is_none() {
        return Err("Add a file extension such as .mp4".to_string());
    }
    let out = folder.join(name);
    let abs = |p: &Path| {
        std::fs::canonicalize(p)
            .or_else(|_| std::path::absolute(p))
            .unwrap_or_else(|_| p.to_path_buf())
    };
    let same = if cfg!(windows) {
        abs(&out).to_string_lossy().to_lowercase() == abs(input).to_string_lossy().to_lowercase()
    } else {
        abs(&out) == abs(input)
    };
    if same {
        return Err("Choose a different name: this would overwrite the video you're cutting from".to_string());
    }
    Ok(out)
}

/// Pure function building ffmpeg CLI arguments for clip export.
pub fn build_clip_args(
    input: &Path,
    output: &Path,
    range: ClipRange,
    mode: ClipExportMode,
    include_audio: bool,
) -> Vec<String> {
    let mut args = Vec::new();
    let dur = (range.end - range.start).max(0.0);

    // Non-interactive overwrite confirmation is handled by the application UI.
    args.push("-y".to_string());

    match mode {
        ClipExportMode::Fast => {
            // Fast: -ss <start> <ffmpeg_input_args(input)> -t <dur> -map 0:v? -map 0:a? -c copy -avoid_negative_ts make_zero
            args.push("-ss".to_string());
            args.push(format!("{:.3}", range.start));
            args.extend(crate::media::ffmpeg_input_args(input));
            args.push("-t".to_string());
            args.push(format!("{:.3}", dur));
            args.push("-map".to_string());
            args.push("0:v?".to_string());
            if include_audio {
                args.push("-map".to_string());
                args.push("0:a?".to_string());
            } else {
                args.push("-an".to_string());
            }
            args.push("-c".to_string());
            args.push("copy".to_string());
            args.push("-avoid_negative_ts".to_string());
            args.push("make_zero".to_string());
        }
        ClipExportMode::Precise => {
            // Input seeking (-ss before -i) is frame-accurate when re-encoding and avoids
            // decoding everything before the clip.
            args.push("-ss".to_string());
            args.push(format!("{:.3}", range.start));
            args.extend(crate::media::ffmpeg_input_args(input));
            args.push("-t".to_string());
            args.push(format!("{:.3}", dur));
            args.push("-map".to_string());
            args.push("0:v?".to_string());
            if include_audio {
                args.push("-map".to_string());
                args.push("0:a?".to_string());
            }
            args.extend(PRECISE_VIDEO_ARGS.iter().map(|s| s.to_string()));
            if include_audio {
                args.extend(PRECISE_AUDIO_ARGS.iter().map(|s| s.to_string()));
            } else {
                args.push("-an".to_string());
            }
        }
    }

    // Always add: -progress pipe:1 -nostats
    args.push("-progress".to_string());
    args.push("pipe:1".to_string());
    args.push("-nostats".to_string());

    // Destination path
    args.push(output.to_string_lossy().to_string());

    args
}

/// Formats seconds as `hh:mm:ss.mmm`.
pub fn format_time_millis(secs: f64) -> String {
    let total_ms = (secs.max(0.0) * 1000.0).round() as u64;
    let ms = total_ms % 1000;
    let total_s = total_ms / 1000;
    let s = total_s % 60;
    let total_m = total_s / 60;
    let m = total_m % 60;
    let h = total_m / 60;
    format!("{h:02}:{m:02}:{s:02}.{ms:03}")
}

/// Generates a default filename for a clip: `<stem>_clip_<start>-<end>.<ext>`.
pub fn default_clip_filename(input: &Path, range: ClipRange) -> String {
    let stem = input
        .file_stem()
        .and_then(|s| s.to_str())
        .unwrap_or("video");
    let ext = input
        .extension()
        .and_then(|s| s.to_str())
        .unwrap_or("mp4");

    let format_ts = |t: f64| {
        let total_s = t.max(0.0);
        if (total_s.fract()).abs() < 0.01 {
            format!("{:.0}s", total_s)
        } else {
            format!("{:.2}s", total_s)
        }
    };

    let start_str = format_ts(range.start);
    let end_str = format_ts(range.end);
    format!("{stem}_clip_{start_str}-{end_str}.{ext}")
}
