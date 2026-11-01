use std::path::{Path, PathBuf};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum RepeatMode {
    #[default]
    Off,
    All,
    One,
}

impl RepeatMode {
    pub fn name(&self) -> &'static str {
        match self {
            Self::Off => "Repeat: Off",
            Self::All => "Repeat: All",
            Self::One => "Repeat: One",
        }
    }

    pub fn cycle(&self) -> Self {
        match self {
            Self::Off => Self::All,
            Self::All => Self::One,
            Self::One => Self::Off,
        }
    }
}

#[derive(Debug, Clone)]
pub struct PlaylistItem {
    pub path: PathBuf,
    pub title: String,
}

#[derive(Default)]
pub struct Playlist {
    pub items: Vec<PlaylistItem>,
    pub current_index: Option<usize>,
    pub repeat_mode: RepeatMode,
    pub is_shuffle: bool,
    pub filter_query: String,
}

impl Playlist {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn add<P: AsRef<Path>>(&mut self, path: P) {
        let p = path.as_ref().to_path_buf();
        let title = p
            .file_name()
            .map(|f| f.to_string_lossy().to_string())
            .unwrap_or_else(|| "Unknown Media".into());

        self.items.push(PlaylistItem { path: p, title });
    }

    pub fn add_multiple<P: AsRef<Path>>(&mut self, paths: &[P]) {
        for p in paths {
            self.add(p);
        }
    }

    pub fn remove(&mut self, index: usize) {
        if index < self.items.len() {
            self.items.remove(index);
            if let Some(cur) = self.current_index {
                if cur == index {
                    if self.items.is_empty() {
                        self.current_index = None;
                    } else if cur >= self.items.len() {
                        self.current_index = Some(self.items.len() - 1);
                    }
                } else if cur > index {
                    self.current_index = Some(cur - 1);
                }
            }
        }
    }

    pub fn clear(&mut self) {
        self.items.clear();
        self.current_index = None;
    }

    pub fn current_item(&self) -> Option<&PlaylistItem> {
        let idx = self.current_index?;
        self.items.get(idx)
    }

    pub fn next(&mut self) -> Option<&PlaylistItem> {
        if self.items.is_empty() {
            return None;
        }

        if self.repeat_mode == RepeatMode::One {
            return self.current_item();
        }

        if self.is_shuffle && self.items.len() > 1 {
            use std::time::{SystemTime, UNIX_EPOCH};
            let nanos = SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap_or_default()
                .subsec_nanos() as usize;
            let next_idx = nanos % self.items.len();
            self.current_index = Some(next_idx);
            return self.current_item();
        }

        let next_idx = match self.current_index {
            Some(cur) => {
                if cur + 1 < self.items.len() {
                    cur + 1
                } else if self.repeat_mode == RepeatMode::All {
                    0
                } else {
                    return None;
                }
            }
            None => 0,
        };

        self.current_index = Some(next_idx);
        self.current_item()
    }

    pub fn prev(&mut self) -> Option<&PlaylistItem> {
        if self.items.is_empty() {
            return None;
        }

        if self.repeat_mode == RepeatMode::One {
            return self.current_item();
        }

        let prev_idx = match self.current_index {
            Some(cur) => {
                if cur > 0 {
                    cur - 1
                } else if self.repeat_mode == RepeatMode::All {
                    self.items.len() - 1
                } else {
                    0
                }
            }
            None => 0,
        };

        self.current_index = Some(prev_idx);
        self.current_item()
    }

    pub fn is_supported_media_extension(ext: &str) -> bool {
        let e = ext.to_lowercase();
        matches!(
            e.as_str(),
            "mp4"
                | "mkv"
                | "avi"
                | "webm"
                | "mov"
                | "flv"
                | "wmv"
                | "ts"
                | "m4v"
                | "3gp"
                | "mp3"
                | "flac"
                | "wav"
                | "aac"
                | "ogg"
                | "opus"
                | "m4a"
                | "wma"
                | "alac"
        )
    }

    pub fn is_supported_subtitle_extension(ext: &str) -> bool {
        let e = ext.to_lowercase();
        matches!(e.as_str(), "srt" | "vtt" | "ass" | "sub")
    }
}
