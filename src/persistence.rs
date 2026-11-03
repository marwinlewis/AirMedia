//! Session state that survives restarts: recent files, the playlist, equalizer settings and
//! per-file resume positions. Stored as JSON next to `preferences.json`.

use crate::media::is_url;
use crate::playlist::RepeatMode;
use crate::ui::equalizer_dialog::EqualizerState;
use std::path::{Path, PathBuf};

pub const MAX_RECENT_FILES: usize = 15;
pub const MAX_RESUME_ENTRIES: usize = 200;
/// Positions earlier than this aren't worth resuming.
pub const RESUME_MIN_SECS: f64 = 30.0;
/// Past this fraction of the duration the item counts as finished.
pub const RESUME_MAX_FRACTION: f64 = 0.95;

/// Per-user configuration directory (`%APPDATA%\airmedia`, `$XDG_CONFIG_HOME/airmedia` or
/// `~/.config/airmedia`).
pub fn config_dir() -> Option<PathBuf> {
    #[cfg(windows)]
    {
        std::env::var_os("APPDATA").map(|p| PathBuf::from(p).join("airmedia"))
    }
    #[cfg(not(windows))]
    {
        std::env::var_os("XDG_CONFIG_HOME")
            .map(PathBuf::from)
            .filter(|p| p.is_absolute())
            .or_else(|| std::env::var_os("HOME").map(|h| PathBuf::from(h).join(".config")))
            .map(|p| p.join("airmedia"))
    }
}

/// Hand-edited JSON (e.g. saved by Notepad or PowerShell) may start with a UTF-8 BOM, which
/// serde_json rejects.
pub fn strip_bom(s: &str) -> &str {
    s.strip_prefix('\u{feff}').unwrap_or(s)
}

/// Writes `contents` via a temporary file + rename so a crash never leaves a truncated file.
pub fn write_atomic(path: &Path, contents: &str) -> std::io::Result<()> {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    let tmp = path.with_extension("json.tmp");
    std::fs::write(&tmp, contents)?;
    std::fs::rename(&tmp, path)
}

#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct ResumeEntry {
    pub path: PathBuf,
    pub position_secs: f64,
}

#[derive(Debug, Clone, Default, serde::Serialize, serde::Deserialize)]
#[serde(default)]
pub struct SessionState {
    /// Most recent first.
    pub recent_files: Vec<PathBuf>,
    pub playlist: Vec<PathBuf>,
    pub playlist_index: Option<usize>,
    pub repeat_mode: RepeatMode,
    pub shuffle: bool,
    pub equalizer: EqualizerState,
    /// Least recently updated first.
    pub resume: Vec<ResumeEntry>,
}

impl SessionState {
    pub fn file_path() -> Option<PathBuf> {
        config_dir().map(|d| d.join("session.json"))
    }

    pub fn load() -> Self {
        let mut state: Self = Self::file_path()
            .and_then(|p| std::fs::read_to_string(p).ok())
            .and_then(|s| serde_json::from_str(strip_bom(&s)).ok())
            .unwrap_or_default();
        state.recent_files.retain(|p| is_url(p) || p.exists());
        state.recent_files.truncate(MAX_RECENT_FILES);
        state
    }

    /// Serializes the state; returns the JSON so callers can skip redundant writes.
    pub fn to_json(&self) -> String {
        serde_json::to_string_pretty(self).unwrap_or_default()
    }

    pub fn save_json(json: &str) {
        if let Some(path) = Self::file_path() {
            if let Err(e) = write_atomic(&path, json) {
                eprintln!("Failed to save session to {}: {e}", path.display());
            }
        }
    }

    pub fn push_recent(&mut self, path: &Path) {
        self.recent_files.retain(|p| p != path);
        self.recent_files.insert(0, path.to_path_buf());
        self.recent_files.truncate(MAX_RECENT_FILES);
    }

    /// Remembers where playback of `path` stopped, or forgets it when the position is too
    /// close to either end to be worth resuming.
    pub fn record_position(&mut self, path: &Path, position_secs: f64, duration_secs: f64) {
        self.resume.retain(|e| e.path != path);
        let near_end = duration_secs > 0.0 && position_secs >= duration_secs * RESUME_MAX_FRACTION;
        if position_secs.is_finite() && position_secs >= RESUME_MIN_SECS && !near_end {
            self.resume.push(ResumeEntry {
                path: path.to_path_buf(),
                position_secs,
            });
            if self.resume.len() > MAX_RESUME_ENTRIES {
                let excess = self.resume.len() - MAX_RESUME_ENTRIES;
                self.resume.drain(..excess);
            }
        }
    }

    pub fn resume_position(&self, path: &Path) -> Option<f64> {
        self.resume.iter().find(|e| e.path == path).map(|e| e.position_secs)
    }
}
