use super::create_hidden_command;
use std::fs;
use std::path::Path;

#[derive(Debug, Clone)]
pub struct SubtitleCue {
    pub start_ms: u64,
    pub end_ms: u64,
    pub text: String,
}

#[derive(Debug, Clone, Default)]
pub struct SubtitleTrack {
    pub id: usize,
    pub name: String,
    pub language: String,
    pub is_external: bool,
    pub cues: Vec<SubtitleCue>,
}

impl SubtitleTrack {
    pub fn from_file<P: AsRef<Path>>(path: P) -> Result<Self, String> {
        let path_ref = path.as_ref();
        let content = fs::read_to_string(path_ref)
            .map_err(|e| format!("Failed to read subtitle file: {e}"))?;

        let cues = if path_ref.extension().map_or(false, |ext| ext == "vtt") {
            parse_vtt(&content)
        } else {
            parse_srt(&content)
        };

        let filename = path_ref
            .file_name()
            .map(|f| f.to_string_lossy().to_string())
            .unwrap_or_else(|| "External Subtitle".into());

        Ok(SubtitleTrack {
            id: 9999, // external marker
            name: filename,
            language: "External".into(),
            is_external: true,
            cues,
        })
    }

    pub fn from_embedded<P: AsRef<Path>>(
        media_path: P,
        stream_index: usize,
        name: String,
        lang: String,
    ) -> Result<Self, String> {
        let path_str = media_path.as_ref().to_string_lossy().to_string();
        let map_arg = format!("0:{stream_index}");

        let output = create_hidden_command("ffmpeg")
            .args([
                "-v",
                "quiet",
                "-i",
                &path_str,
                "-map",
                &map_arg,
                "-f",
                "srt",
                "-",
            ])
            .output()
            .map_err(|e| format!("Failed to extract embedded subtitle: {e}"))?;

        if !output.status.success() {
            return Err("Embedded subtitle extraction failed".into());
        }

        let srt_text = String::from_utf8_lossy(&output.stdout);
        let cues = parse_srt(&srt_text);

        Ok(SubtitleTrack {
            id: stream_index,
            name,
            language: lang,
            is_external: false,
            cues,
        })
    }

    pub fn get_text_at(&self, time_ms: i64) -> Option<String> {
        if time_ms < 0 {
            return None;
        }
        let t = time_ms as u64;
        let mut active = Vec::new();
        for cue in &self.cues {
            if t >= cue.start_ms && t <= cue.end_ms {
                active.push(cue.text.as_str());
            }
        }
        if active.is_empty() {
            None
        } else {
            Some(active.join("\n"))
        }
    }
}

pub fn parse_srt(content: &str) -> Vec<SubtitleCue> {
    let mut cues = Vec::new();
    let normalized = content.replace("\r\n", "\n").replace('\r', "\n");
    let blocks = normalized.split("\n\n");

    for block in blocks {
        let lines: Vec<&str> = block.lines().map(|l| l.trim()).filter(|l| !l.is_empty()).collect();
        if lines.is_empty() {
            continue;
        }

        // Line 0 may be index number, or timestamps if index is omitted
        let (time_line, text_start_idx) = if lines[0].contains("-->") {
            (lines[0], 1)
        } else if lines.len() > 1 && lines[1].contains("-->") {
            (lines[1], 2)
        } else {
            continue;
        };

        if let Some((start, end)) = parse_timestamp_range(time_line) {
            let text = lines[text_start_idx..].join("\n");
            // Clean basic html tags like <i>, </i>, <font>, etc.
            let clean_text = strip_tags(&text);
            if !clean_text.is_empty() {
                cues.push(SubtitleCue {
                    start_ms: start,
                    end_ms: end,
                    text: clean_text,
                });
            }
        }
    }

    cues
}

pub fn parse_vtt(content: &str) -> Vec<SubtitleCue> {
    let mut cues = Vec::new();
    let normalized = content.replace("\r\n", "\n").replace('\r', "\n");
    let blocks = normalized.split("\n\n");

    for block in blocks {
        let lines: Vec<&str> = block.lines().map(|l| l.trim()).filter(|l| !l.is_empty()).collect();
        if lines.is_empty() || lines[0].starts_with("WEBVTT") || lines[0].starts_with("NOTE") {
            continue;
        }

        let (time_line, text_start_idx) = if lines[0].contains("-->") {
            (lines[0], 1)
        } else if lines.len() > 1 && lines[1].contains("-->") {
            (lines[1], 2)
        } else {
            continue;
        };

        if let Some((start, end)) = parse_timestamp_range(time_line) {
            let text = lines[text_start_idx..].join("\n");
            let clean_text = strip_tags(&text);
            if !clean_text.is_empty() {
                cues.push(SubtitleCue {
                    start_ms: start,
                    end_ms: end,
                    text: clean_text,
                });
            }
        }
    }

    cues
}

fn parse_timestamp_range(line: &str) -> Option<(u64, u64)> {
    let parts: Vec<&str> = line.split("-->").collect();
    if parts.len() != 2 {
        return None;
    }

    let start_str = parts[0].trim();
    // VTT timestamp line might have alignment info after end time (e.g. 00:01.000 align:start)
    let end_str = parts[1].trim().split_whitespace().next()?;

    let start_ms = parse_timestamp(start_str)?;
    let end_ms = parse_timestamp(end_str)?;

    Some((start_ms, end_ms))
}

fn parse_timestamp(s: &str) -> Option<u64> {
    // formats: HH:MM:SS,mmm or HH:MM:SS.mmm or MM:SS.mmm
    let s = s.replace(',', ".");
    let colon_count = s.matches(':').count();

    if colon_count == 2 {
        // HH:MM:SS.mmm
        let parts: Vec<&str> = s.split(':').collect();
        let hours: u64 = parts[0].parse().ok()?;
        let minutes: u64 = parts[1].parse().ok()?;
        let sec_parts: Vec<&str> = parts[2].split('.').collect();
        let seconds: u64 = sec_parts[0].parse().ok()?;
        let millis: u64 = if sec_parts.len() > 1 {
            let m_str = format!("{:0<3}", sec_parts[1]);
            m_str[..3.min(m_str.len())].parse().unwrap_or(0)
        } else {
            0
        };

        Some(hours * 3600_000 + minutes * 60_000 + seconds * 1_000 + millis)
    } else if colon_count == 1 {
        // MM:SS.mmm
        let parts: Vec<&str> = s.split(':').collect();
        let minutes: u64 = parts[0].parse().ok()?;
        let sec_parts: Vec<&str> = parts[1].split('.').collect();
        let seconds: u64 = sec_parts[0].parse().ok()?;
        let millis: u64 = if sec_parts.len() > 1 {
            let m_str = format!("{:0<3}", sec_parts[1]);
            m_str[..3.min(m_str.len())].parse().unwrap_or(0)
        } else {
            0
        };

        Some(minutes * 60_000 + seconds * 1_000 + millis)
    } else {
        None
    }
}

fn strip_tags(s: &str) -> String {
    let mut result = String::with_capacity(s.len());
    let mut inside = false;
    for c in s.chars() {
        if c == '<' {
            inside = true;
        } else if c == '>' {
            inside = false;
        } else if !inside {
            result.push(c);
        }
    }
    result
}
