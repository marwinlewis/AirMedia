use super::{create_hidden_command, ffmpeg_input_args, is_url, output_with_timeout, MAX_VIDEO_DIMENSION};
use serde::Deserialize;
use std::path::Path;
use std::time::Duration;

#[derive(Debug, Clone, Deserialize, Default)]
pub struct MediaInfo {
    #[serde(default)]
    pub streams: Vec<StreamInfo>,
    #[serde(default)]
    pub format: FormatInfo,
}

#[derive(Debug, Clone, Deserialize, Default)]
pub struct FormatInfo {
    #[serde(default)]
    pub filename: String,
    #[serde(default)]
    pub format_name: String,
    #[serde(default)]
    pub format_long_name: String,
    #[serde(default)]
    pub duration: Option<String>,
    #[serde(default)]
    pub size: Option<String>,
    #[serde(default)]
    pub bit_rate: Option<String>,
    #[serde(default)]
    pub tags: Option<serde_json::Value>,
}

#[derive(Debug, Clone, Deserialize, Default)]
pub struct StreamInfo {
    pub index: usize,
    #[serde(default)]
    pub codec_name: Option<String>,
    #[serde(default)]
    pub codec_long_name: Option<String>,
    #[serde(default)]
    pub codec_type: String, // "video", "audio", "subtitle"
    #[serde(default)]
    pub width: Option<u32>,
    #[serde(default)]
    pub height: Option<u32>,
    #[serde(default)]
    pub r_frame_rate: Option<String>,
    #[serde(default)]
    pub avg_frame_rate: Option<String>,
    #[serde(default)]
    pub duration: Option<String>,
    #[serde(default)]
    pub bit_rate: Option<String>,
    #[serde(default)]
    pub sample_rate: Option<String>,
    #[serde(default)]
    pub channels: Option<u32>,
    #[serde(default)]
    pub channel_layout: Option<String>,
    #[serde(default)]
    pub tags: Option<serde_json::Value>,
    #[serde(default)]
    pub side_data_list: Option<Vec<serde_json::Value>>,
}

impl StreamInfo {
    /// Display rotation in degrees (display matrix side data, or the legacy `rotate` tag).
    pub fn rotation_degrees(&self) -> i64 {
        let from_side_data = self.side_data_list.as_ref().and_then(|list| {
            list.iter().find_map(|sd| sd.get("rotation").and_then(|r| r.as_f64().or_else(|| r.as_str()?.parse().ok())))
        });
        let from_tag = || {
            self.tags
                .as_ref()
                .and_then(|t| t.get("rotate"))
                .and_then(|r| r.as_str())
                .and_then(|r| r.parse::<f64>().ok())
        };
        from_side_data.or_else(from_tag).map_or(0, |r| r.round() as i64)
    }
}

impl MediaInfo {
    pub fn probe<P: AsRef<Path>>(path: P) -> Result<Self, String> {
        let path_ref = path.as_ref();
        let path_str = path_ref.to_string_lossy().to_string();

        let mut cmd = create_hidden_command("ffprobe");
        cmd.args(["-v", "error", "-print_format", "json", "-show_format", "-show_streams"]);
        cmd.args(ffmpeg_input_args(path_ref));
        // Never let a dead stream or a pathological file freeze the caller indefinitely.
        let timeout = if is_url(path_ref) { Duration::from_secs(15) } else { Duration::from_secs(30) };
        let output = output_with_timeout(cmd, timeout)
            .map_err(|e| format!("Failed to run ffprobe on {path_str}: {e}. Ensure ffmpeg/ffprobe is installed."))?;

        if !output.status.success() {
            let err = String::from_utf8_lossy(&output.stderr);
            return Err(format!("ffprobe error: {}", err.trim()));
        }

        let json_str = String::from_utf8_lossy(&output.stdout);
        let info: MediaInfo = serde_json::from_str(&json_str)
            .map_err(|e| format!("Failed to parse ffprobe json: {e}"))?;

        Ok(info)
    }

    pub fn duration_seconds(&self) -> f64 {
        if let Some(dur_str) = &self.format.duration {
            if let Ok(dur) = dur_str.parse::<f64>() {
                return dur;
            }
        }

        for stream in &self.streams {
            if let Some(dur_str) = &stream.duration {
                if let Ok(dur) = dur_str.parse::<f64>() {
                    return dur;
                }
            }
        }

        0.0
    }

    pub fn primary_video_stream(&self) -> Option<&StreamInfo> {
        self.streams.iter().find(|s| s.codec_type == "video")
    }

    pub fn audio_streams(&self) -> Vec<&StreamInfo> {
        self.streams.iter().filter(|s| s.codec_type == "audio").collect()
    }

    pub fn subtitle_streams(&self) -> Vec<&StreamInfo> {
        self.streams.iter().filter(|s| s.codec_type == "subtitle").collect()
    }

    pub fn title(&self) -> Option<String> {
        let tags = self.format.tags.as_ref()?;
        tags.get("title")
            .or_else(|| tags.get("TITLE"))
            .or_else(|| tags.get("Title"))
            .and_then(|v| v.as_str())
            .map(|s| s.to_string())
    }

    pub fn artist(&self) -> Option<String> {
        let tags = self.format.tags.as_ref()?;
        tags.get("artist")
            .or_else(|| tags.get("ARTIST"))
            .or_else(|| tags.get("Artist"))
            .or_else(|| tags.get("album_artist"))
            .or_else(|| tags.get("ALBUM_ARTIST"))
            .and_then(|v| v.as_str())
            .map(|s| s.to_string())
    }

    pub fn album(&self) -> Option<String> {
        let tags = self.format.tags.as_ref()?;
        tags.get("album")
            .or_else(|| tags.get("ALBUM"))
            .or_else(|| tags.get("Album"))
            .and_then(|v| v.as_str())
            .map(|s| s.to_string())
    }

    /// Rejects media whose metadata would make the player misbehave (absurd frame sizes, or
    /// nothing playable at all).
    pub fn validate(&self) -> Result<(), String> {
        if let Some(v) = self.primary_video_stream() {
            if let (Some(w), Some(h)) = (v.width, v.height) {
                if w > MAX_VIDEO_DIMENSION || h > MAX_VIDEO_DIMENSION {
                    return Err(format!(
                        "video size {w}x{h} exceeds the supported maximum of {MAX_VIDEO_DIMENSION}px"
                    ));
                }
            }
        }
        if self.primary_video_stream().is_none() && self.audio_streams().is_empty() {
            return Err("no audio or video streams found".to_string());
        }
        Ok(())
    }

    /// Displayed frame size: swaps width/height for 90°/270° rotated video (ffmpeg autorotates
    /// the decoded frames). Returns `None` for missing, zero or oversized dimensions.
    pub fn video_dimensions(&self) -> Option<(u32, u32)> {
        let v = self.primary_video_stream()?;
        let (w, h) = (v.width?, v.height?);
        if w == 0 || h == 0 || w > MAX_VIDEO_DIMENSION || h > MAX_VIDEO_DIMENSION {
            return None;
        }
        if v.rotation_degrees().rem_euclid(180) == 90 {
            Some((h, w))
        } else {
            Some((w, h))
        }
    }

    pub fn fps(&self) -> f64 {
        if let Some(v) = self.primary_video_stream() {
            if let Some(rate_str) = &v.avg_frame_rate {
                if let Some(fps) = parse_fraction(rate_str) {
                    if fps > 0.0 {
                        return fps;
                    }
                }
            }
            if let Some(rate_str) = &v.r_frame_rate {
                if let Some(fps) = parse_fraction(rate_str) {
                    if fps > 0.0 {
                        return fps;
                    }
                }
            }
        }
        30.0 // reasonable default fallback
    }
}

fn parse_fraction(s: &str) -> Option<f64> {
    let parts: Vec<&str> = s.split('/').collect();
    if parts.len() == 2 {
        let num: f64 = parts[0].trim().parse().ok()?;
        let den: f64 = parts[1].trim().parse().ok()?;
        if den != 0.0 {
            return Some(num / den);
        }
    } else if let Ok(val) = s.parse::<f64>() {
        return Some(val);
    }
    None
}
