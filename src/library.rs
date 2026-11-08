//! The browsable view of what the user has: the playlist, recently played files and saved
//! resume positions merged into one de-duplicated list, plus the rows the home screen shows.
//! Pure data, so it can be rebuilt (and tested) without any UI.

use crate::media::is_url;
use crate::playlist::{natural_cmp, Playlist};
use std::collections::hash_map::DefaultHasher;
use std::collections::{BTreeMap, HashMap};
use std::hash::{Hash, Hasher};
use std::path::{Path, PathBuf};

/// Items shown in the "pick up where you left off" row.
const MAX_RESUME_ROW: usize = 20;
/// Folder rows only appear for folders holding at least this many items...
const MIN_FOLDER_ROW_ITEMS: usize = 3;
/// ...and at most this many of them.
const MAX_FOLDER_ROWS: usize = 4;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum MediaKind {
    Video,
    Audio,
    Stream,
}

impl MediaKind {
    pub fn of(path: &Path) -> Self {
        if is_url(path) {
            return Self::Stream;
        }
        let ext = path.extension().and_then(|e| e.to_str()).unwrap_or("").to_lowercase();
        match ext.as_str() {
            "mp3" | "flac" | "wav" | "aac" | "ogg" | "opus" | "m4a" | "wma" | "alac" | "aiff" | "ape" | "wv" | "mka" => Self::Audio,
            _ => Self::Video,
        }
    }

    pub fn label(self) -> &'static str {
        match self {
            Self::Video => "Video",
            Self::Audio => "Music",
            Self::Stream => "Stream",
        }
    }
}

/// A readable title from a file name or URL: `The.Long_Road.2019.mkv` → `The Long Road 2019`.
/// Names that already contain spaces keep their dots and underscores.
pub fn display_title(path: &Path) -> String {
    if is_url(path) {
        let full = path.to_string_lossy();
        let no_query = full.split(['?', '#']).next().unwrap_or(&full);
        // Everything after `scheme://host/`; a bare host has no useful name of its own.
        let url_path = no_query.split_once("://").and_then(|(_, rest)| rest.split_once('/')).map_or("", |(_, p)| p);
        let seg = url_path.trim_end_matches('/').rsplit('/').next().unwrap_or("");
        if seg.is_empty() {
            return full.to_string();
        }
        return clean_stem(seg.rsplit_once('.').map_or(seg, |(stem, _)| stem));
    }
    match path.file_stem() {
        Some(stem) => clean_stem(&stem.to_string_lossy()),
        None => path.to_string_lossy().to_string(),
    }
}

fn clean_stem(stem: &str) -> String {
    let cleaned = if stem.contains(' ') {
        stem.to_string()
    } else {
        stem.replace(['.', '_'], " ")
    };
    let joined = cleaned.split_whitespace().collect::<Vec<_>>().join(" ");
    if joined.is_empty() {
        stem.to_string()
    } else {
        joined
    }
}

#[derive(Debug, Clone)]
pub struct LibraryEntry {
    pub path: PathBuf,
    pub title: String,
    pub kind: MediaKind,
    pub in_playlist: bool,
    /// In the recent list or has a saved position.
    pub in_history: bool,
    /// Lower-cased title and folder, for search.
    search_key: String,
}

impl LibraryEntry {
    fn new(path: &Path) -> Self {
        let title = display_title(path);
        let folder = folder_name(path).unwrap_or_default();
        let raw_name = path.file_name().map(|n| n.to_string_lossy().to_string()).unwrap_or_default();
        let search_key = format!("{title} {raw_name} {folder}").to_lowercase();
        Self {
            path: path.to_path_buf(),
            title,
            kind: MediaKind::of(path),
            in_playlist: false,
            in_history: false,
            search_key,
        }
    }

    /// Upper-case file extension (`MKV`), or `None` for streams.
    pub fn format_label(&self) -> Option<String> {
        if self.kind == MediaKind::Stream {
            return None;
        }
        self.path.extension().map(|e| e.to_string_lossy().to_uppercase())
    }
}

fn folder_name(path: &Path) -> Option<String> {
    if is_url(path) {
        return None;
    }
    path.parent()?.file_name().map(|n| n.to_string_lossy().to_string())
}

#[derive(Debug, Clone)]
pub struct LibraryRow {
    pub title: String,
    /// Indices into [`Library::entries`].
    pub items: Vec<usize>,
}

/// Why an item is the one featured in the hero banner.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FeaturedReason {
    NowPlaying,
    Continue,
    UpNext,
    Recent,
}

#[derive(Debug, Default)]
pub struct Library {
    pub entries: Vec<LibraryEntry>,
    pub rows: Vec<LibraryRow>,
    pub videos: Vec<usize>,
    pub music: Vec<usize>,
    resume_row: Vec<usize>,
    index: HashMap<PathBuf, usize>,
}

impl Library {
    /// Merges the sources. `resume` and `recent` are newest first; `exists` filters local files
    /// out of the history rows when they are gone (the playlist keeps showing them, as it does).
    pub fn build(
        playlist: &[PathBuf],
        recent: &[PathBuf],
        resume: &[PathBuf],
        exists: &mut dyn FnMut(&Path) -> bool,
    ) -> Self {
        let mut lib = Self::default();

        for path in playlist {
            let idx = lib.entry(path);
            lib.entries[idx].in_playlist = true;
        }
        let playlist_row: Vec<usize> = playlist.iter().filter_map(|p| lib.index.get(p).copied()).collect();

        let mut alive = |p: &Path| is_url(p) || exists(p);

        let mut resume_row = Vec::new();
        for path in resume.iter().filter(|p| alive(p)).take(MAX_RESUME_ROW) {
            let idx = lib.entry(path);
            lib.entries[idx].in_history = true;
            resume_row.push(idx);
        }

        let mut recent_row = Vec::new();
        for path in recent.iter().filter(|p| alive(p)) {
            let idx = lib.entry(path);
            lib.entries[idx].in_history = true;
            if !resume_row.contains(&idx) {
                recent_row.push(idx);
            }
        }

        for (idx, entry) in lib.entries.iter().enumerate() {
            match entry.kind {
                MediaKind::Video | MediaKind::Stream => lib.videos.push(idx),
                MediaKind::Audio => lib.music.push(idx),
            }
        }

        let mut rows = Vec::new();
        if !resume_row.is_empty() {
            rows.push(LibraryRow { title: "Pick Up Where You Left Off".into(), items: resume_row.clone() });
        }
        if !playlist_row.is_empty() {
            rows.push(LibraryRow { title: "Your Playlist".into(), items: playlist_row });
        }
        if !recent_row.is_empty() {
            rows.push(LibraryRow { title: "Recently Played".into(), items: recent_row });
        }
        // Splitting by type only says something when there are both types.
        if !lib.videos.is_empty() && !lib.music.is_empty() {
            rows.push(LibraryRow { title: "Videos".into(), items: lib.videos.clone() });
            rows.push(LibraryRow { title: "Music".into(), items: lib.music.clone() });
        }
        rows.extend(lib.folder_rows());

        lib.rows = rows;
        lib.resume_row = resume_row;
        lib
    }

    fn entry(&mut self, path: &Path) -> usize {
        if let Some(idx) = self.index.get(path) {
            return *idx;
        }
        let idx = self.entries.len();
        self.entries.push(LibraryEntry::new(path));
        self.index.insert(path.to_path_buf(), idx);
        idx
    }

    fn folder_rows(&self) -> Vec<LibraryRow> {
        let mut folders: BTreeMap<PathBuf, Vec<usize>> = BTreeMap::new();
        for (idx, entry) in self.entries.iter().enumerate() {
            if is_url(&entry.path) {
                continue;
            }
            if let Some(parent) = entry.path.parent() {
                folders.entry(parent.to_path_buf()).or_default().push(idx);
            }
        }
        // A single folder would only repeat the playlist row.
        if folders.len() < 2 {
            return Vec::new();
        }
        let mut rows: Vec<(String, Vec<usize>)> = folders
            .into_iter()
            .filter(|(_, items)| items.len() >= MIN_FOLDER_ROW_ITEMS)
            .filter_map(|(folder, items)| {
                let name = folder.file_name()?.to_string_lossy().to_string();
                Some((name, items))
            })
            .collect();
        rows.sort_by(|a, b| b.1.len().cmp(&a.1.len()).then_with(|| natural_cmp(&a.0, &b.0)));
        rows.truncate(MAX_FOLDER_ROWS);
        rows.into_iter()
            .map(|(name, mut items)| {
                items.sort_by(|a, b| natural_cmp(&self.entries[*a].title, &self.entries[*b].title));
                LibraryRow { title: format!("From {name}"), items }
            })
            .collect()
    }

    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    pub fn position(&self, path: &Path) -> Option<usize> {
        self.index.get(path).copied()
    }

    /// The item the hero banner should feature: what is playing, else what can be resumed, else
    /// what comes next, else the latest thing played.
    pub fn featured(&self, now_playing: Option<&Path>, up_next: Option<&Path>) -> Option<(usize, FeaturedReason)> {
        if let Some(idx) = now_playing.and_then(|p| self.position(p)) {
            return Some((idx, FeaturedReason::NowPlaying));
        }
        if let Some(idx) = self.resume_row.first() {
            return Some((*idx, FeaturedReason::Continue));
        }
        if let Some(idx) = up_next.and_then(|p| self.position(p)) {
            return Some((idx, FeaturedReason::UpNext));
        }
        if let Some(idx) = self.entries.iter().position(|e| e.in_playlist) {
            return Some((idx, FeaturedReason::UpNext));
        }
        (!self.entries.is_empty()).then_some((0, FeaturedReason::Recent))
    }

    /// Entries matching every whitespace-separated term of `query` (case-insensitive).
    pub fn search(&self, query: &str) -> Vec<usize> {
        let query = query.to_lowercase();
        let terms: Vec<&str> = query.split_whitespace().collect();
        if terms.is_empty() {
            return Vec::new();
        }
        self.entries
            .iter()
            .enumerate()
            .filter(|(_, e)| terms.iter().all(|t| e.search_key.contains(t)))
            .map(|(i, _)| i)
            .collect()
    }
}

/// Fingerprint of everything [`Library::build`] reads, so it only reruns when something changed.
pub fn library_signature(playlist: &Playlist, recent: &[PathBuf], resume: &[PathBuf]) -> u64 {
    let mut h = DefaultHasher::new();
    for item in &playlist.items {
        item.path.hash(&mut h);
    }
    0xFFu8.hash(&mut h);
    recent.hash(&mut h);
    0xFFu8.hash(&mut h);
    resume.hash(&mut h);
    h.finish()
}

/// Compact duration for cards and banners: `1h 42m`, `42m`, `3:07`.
pub fn format_duration_short(secs: f64) -> String {
    let total = secs.max(0.0).round() as u64;
    let (h, m, s) = (total / 3600, (total % 3600) / 60, total % 60);
    if h > 0 {
        if m > 0 {
            format!("{h}h {m}m")
        } else {
            format!("{h}h")
        }
    } else if m >= 10 {
        format!("{m}m")
    } else {
        format!("{m}:{s:02}")
    }
}
