use super::create_hidden_command;
use serde::Deserialize;
use std::path::Path;

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
}

impl MediaInfo {
    pub fn probe<P: AsRef<Path>>(path: P) -> Result<Self, String> {
        let path_ref = path.as_ref();
        let path_str = path_ref.to_string_lossy().to_string();

        let output = create_hidden_command("ffprobe")
            .args([
                "-v",
                "quiet",
                "-print_format",
                "json",
                "-show_format",
                "-show_streams",
                &path_str,
            ])
            .output()
            .map_err(|e| format!("Failed to run ffprobe: {e}. Ensure ffmpeg/ffprobe is installed."))?;

        if !output.status.success() {
            let err = String::from_utf8_lossy(&output.stderr);
            return Err(format!("ffprobe error: {err}"));
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

    pub fn video_dimensions(&self) -> Option<(u32, u32)> {
        self.primary_video_stream().and_then(|v| {
            if let (Some(w), Some(h)) = (v.width, v.height) {
                Some((w, h))
            } else {
                None
            }
        })
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
