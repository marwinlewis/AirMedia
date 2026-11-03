use super::{create_hidden_command, ffmpeg_input_args, is_url, output_with_timeout};
use std::fs;
use std::path::{Path, PathBuf};
use std::time::Duration;

/// External subtitle files larger than this are rejected rather than read into memory.
pub const MAX_SUBTITLE_FILE_BYTES: u64 = 20 * 1024 * 1024;

/// Subtitle file extensions the player can load.
pub const SUBTITLE_EXTENSIONS: [&str; 4] = ["srt", "vtt", "ass", "ssa"];

/// Image-based subtitle codecs that cannot be converted to text cues.
pub fn is_bitmap_subtitle_codec(codec: Option<&str>) -> bool {
    matches!(
        codec,
        Some("hdmv_pgs_subtitle" | "dvd_subtitle" | "dvb_subtitle" | "xsub" | "dvb_teletext")
    )
}

/// Decodes subtitle bytes: UTF-8 (with or without BOM), UTF-16 with BOM, otherwise
/// Windows-1252/Latin-1 (the usual encoding of legacy .srt files).
pub fn decode_subtitle_bytes(bytes: &[u8]) -> String {
    if let Some(rest) = bytes.strip_prefix(&[0xEF, 0xBB, 0xBF]) {
        return String::from_utf8_lossy(rest).into_owned();
    }
    if bytes.len() >= 2 && (bytes[..2] == [0xFF, 0xFE] || bytes[..2] == [0xFE, 0xFF]) {
        let le = bytes[0] == 0xFF;
        let units: Vec<u16> = bytes[2..]
            .chunks_exact(2)
            .map(|c| if le { u16::from_le_bytes([c[0], c[1]]) } else { u16::from_be_bytes([c[0], c[1]]) })
            .collect();
        return String::from_utf16_lossy(&units);
    }
    match std::str::from_utf8(bytes) {
        Ok(s) => s.to_owned(),
        Err(_) => bytes.iter().map(|&b| b as char).collect(),
    }
}

/// Finds subtitle files next to a local media file that share its name, e.g. `movie.srt`
/// and `movie.en.srt` for `movie.mkv`. Sorted so the exact-name match comes first.
pub fn find_sidecar_subtitles(media: &Path) -> Vec<PathBuf> {
    if is_url(media) {
        return Vec::new();
    }
    let (Some(dir), Some(stem)) = (media.parent(), media.file_stem().and_then(|s| s.to_str())) else {
        return Vec::new();
    };
    let dir = if dir.as_os_str().is_empty() { Path::new(".") } else { dir };
    let Ok(entries) = fs::read_dir(dir) else { return Vec::new() };
    let stem_lower = stem.to_lowercase();

    let mut found: Vec<PathBuf> = entries
        .flatten()
        .map(|e| e.path())
        .filter(|p| {
            let ext_ok = p
                .extension()
                .and_then(|e| e.to_str())
                .is_some_and(|e| SUBTITLE_EXTENSIONS.contains(&e.to_lowercase().as_str()));
            let name_ok = p.file_stem().and_then(|s| s.to_str()).is_some_and(|s| {
                let s = s.to_lowercase();
                s == stem_lower || s.starts_with(&format!("{stem_lower}."))
            });
            ext_ok && name_ok && p.is_file()
        })
        .collect();
    found.sort_by_key(|p| (p.file_stem().map(|s| s.len()).unwrap_or(0), p.clone()));
    found
}

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
        let size = fs::metadata(path_ref)
            .map_err(|e| format!("Failed to read subtitle file: {e}"))?
            .len();
        if size > MAX_SUBTITLE_FILE_BYTES {
            return Err(format!("Subtitle file is too large ({} MB)", size / (1024 * 1024)));
        }

        let ext = path_ref
            .extension()
            .and_then(|e| e.to_str())
            .unwrap_or("")
            .to_lowercase();
        let cues = match ext.as_str() {
            "vtt" => parse_vtt(&decode_subtitle_bytes(&read_file(path_ref)?)),
            // Styled formats are flattened to SRT by ffmpeg.
            "ass" | "ssa" => parse_srt(&convert_to_srt(path_ref, None)?),
            _ => parse_srt(&decode_subtitle_bytes(&read_file(path_ref)?)),
        };
        if cues.is_empty() {
            return Err("No subtitle cues found in file".to_string());
        }

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
        let srt_text = convert_to_srt(media_path.as_ref(), Some(stream_index))?;
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

fn read_file(path: &Path) -> Result<Vec<u8>, String> {
    fs::read(path).map_err(|e| format!("Failed to read subtitle file: {e}"))
}

/// Uses ffmpeg to convert a subtitle stream (or a whole .ass/.ssa file) to SRT text.
fn convert_to_srt(input: &Path, stream_index: Option<usize>) -> Result<String, String> {
    let mut cmd = create_hidden_command("ffmpeg");
    cmd.args(["-v", "error"]);
    cmd.args(ffmpeg_input_args(input));
    let map_arg = format!("0:{}", stream_index.unwrap_or(0));
    let map = if stream_index.is_some() { map_arg.as_str() } else { "0:s:0" };
    cmd.args(["-map", map, "-f", "srt", "-"]);

    let output = output_with_timeout(cmd, Duration::from_secs(120))
        .map_err(|e| format!("Failed to extract subtitles: {e}"))?;
    if !output.status.success() {
        return Err("Subtitle extraction failed".into());
    }
    Ok(decode_subtitle_bytes(&output.stdout))
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
            // Char-based so non-ASCII junk in a crafted file can't split a UTF-8 sequence.
            let digits: String = sec_parts[1].chars().take(3).collect();
            format!("{digits:0<3}").parse().unwrap_or(0)
        } else {
            0
        };

        hours
            .checked_mul(3_600_000)?
            .checked_add(minutes.checked_mul(60_000)?)?
            .checked_add(seconds.checked_mul(1_000)?)?
            .checked_add(millis)
    } else if colon_count == 1 {
        // MM:SS.mmm
        let parts: Vec<&str> = s.split(':').collect();
        let minutes: u64 = parts[0].parse().ok()?;
        let sec_parts: Vec<&str> = parts[1].split('.').collect();
        let seconds: u64 = sec_parts[0].parse().ok()?;
        let millis: u64 = if sec_parts.len() > 1 {
            // Char-based so non-ASCII junk in a crafted file can't split a UTF-8 sequence.
            let digits: String = sec_parts[1].chars().take(3).collect();
            format!("{digits:0<3}").parse().unwrap_or(0)
        } else {
            0
        };

        minutes
            .checked_mul(60_000)?
            .checked_add(seconds.checked_mul(1_000)?)?
            .checked_add(millis)
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
