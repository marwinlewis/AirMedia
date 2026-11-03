use crate::media::is_url;
use crate::media::subtitle::SUBTITLE_EXTENSIONS;
use std::cmp::Ordering;
use std::collections::BTreeSet;
use std::path::{Path, PathBuf};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, serde::Serialize, serde::Deserialize)]
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
    /// Indices selected in the playlist drawer; kept valid by reorder/remove.
    pub selection: BTreeSet<usize>,
    /// Indices played while shuffling, so Previous can walk back and Next avoids repeats.
    shuffle_history: Vec<usize>,
    rng_state: u64,
}

/// Media paths and subtitle files found by [`Playlist::collect_media`].
#[derive(Debug, Default)]
pub struct CollectedMedia {
    pub media: Vec<PathBuf>,
    pub subtitles: Vec<PathBuf>,
}

impl Playlist {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn add<P: AsRef<Path>>(&mut self, path: P) {
        let p = path.as_ref().to_path_buf();
        let title = if is_url(&p) {
            // Use the last URL segment (without query) when there is one, else the full URL.
            let s = p.to_string_lossy();
            let no_query = s.split(['?', '#']).next().unwrap_or(&s);
            no_query
                .trim_end_matches('/')
                .rsplit('/')
                .next()
                .filter(|seg| !seg.is_empty() && !seg.contains(':'))
                .map(str::to_string)
                .unwrap_or_else(|| s.to_string())
        } else {
            p.file_name()
                .map(|f| f.to_string_lossy().to_string())
                .unwrap_or_else(|| "Unknown Media".into())
        };

        self.items.push(PlaylistItem { path: p, title });
    }

    pub fn add_multiple<P: AsRef<Path>>(&mut self, paths: &[P]) {
        for p in paths {
            self.add(p);
        }
    }

    pub fn remove(&mut self, index: usize) {
        self.remove_many(&[index]);
    }

    /// Removes every listed index (duplicates and out-of-range entries are ignored). If the
    /// current item goes, the current index moves to the next surviving item (clamped to the
    /// end), like a single remove.
    pub fn remove_many(&mut self, indices: &[usize]) {
        let len = self.items.len();
        let doomed: BTreeSet<usize> = indices.iter().copied().filter(|i| *i < len).collect();
        if doomed.is_empty() {
            return;
        }
        // new_pos[old] = index after removal, or None if removed.
        let mut new_pos = Vec::with_capacity(len);
        let mut next = 0;
        for i in 0..len {
            if doomed.contains(&i) {
                new_pos.push(None);
            } else {
                new_pos.push(Some(next));
                next += 1;
            }
        }
        let mut i = 0;
        self.items.retain(|_| {
            i += 1;
            !doomed.contains(&(i - 1))
        });

        self.current_index = self.current_index.and_then(|cur| match new_pos.get(cur).copied().flatten() {
            Some(n) => Some(n),
            None if self.items.is_empty() => None,
            // Survivors before `cur` = the slot the next survivor now occupies.
            None => Some(new_pos[..cur].iter().flatten().count().min(self.items.len() - 1)),
        });
        self.remap(|old| new_pos.get(old).copied().flatten());
    }

    /// Moves the items at `indices` (kept in their relative order) so they sit together
    /// before the item that was at `slot` (`slot == len` means the end). Returns the new
    /// index range of the moved block. The current item, selection and shuffle history
    /// follow their items.
    pub fn move_items(&mut self, indices: &[usize], slot: usize) -> std::ops::Range<usize> {
        let len = self.items.len();
        let moving: BTreeSet<usize> = indices.iter().copied().filter(|i| *i < len).collect();
        let slot = slot.min(len);
        let before: Vec<usize> = (0..slot).filter(|i| !moving.contains(i)).collect();
        let after: Vec<usize> = (slot..len).filter(|i| !moving.contains(i)).collect();
        let start = before.len();
        let order: Vec<usize> = before.into_iter().chain(moving.iter().copied()).chain(after).collect();

        let mut new_pos = vec![0; len];
        for (new, old) in order.iter().enumerate() {
            new_pos[*old] = new;
        }
        let mut old_items: Vec<Option<PlaylistItem>> = std::mem::take(&mut self.items).into_iter().map(Some).collect();
        self.items = order.iter().map(|old| old_items[*old].take().expect("each index once")).collect();

        self.current_index = self.current_index.map(|cur| new_pos.get(cur).copied().unwrap_or(cur));
        self.remap(|old| new_pos.get(old).copied());
        start..start + moving.len()
    }

    /// Rewrites selection and shuffle history through `map` (old index -> new, or dropped).
    fn remap(&mut self, map: impl Fn(usize) -> Option<usize>) {
        self.selection = self.selection.iter().filter_map(|i| map(*i)).collect();
        self.shuffle_history = self.shuffle_history.iter().filter_map(|i| map(*i)).collect();
    }

    pub fn clear(&mut self) {
        self.items.clear();
        self.current_index = None;
        self.selection.clear();
        self.shuffle_history.clear();
    }

    pub fn current_item(&self) -> Option<&PlaylistItem> {
        let idx = self.current_index?;
        self.items.get(idx)
    }

    /// Advance after the current item finished playing. Honours Repeat-One by returning the
    /// current item again; otherwise behaves like [`Self::next`].
    pub fn next_auto(&mut self) -> Option<&PlaylistItem> {
        if self.repeat_mode == RepeatMode::One && self.current_item().is_some() {
            return self.current_item();
        }
        self.next()
    }

    /// Manual "Next": always moves to another item (Repeat-One only affects auto-advance).
    pub fn next(&mut self) -> Option<&PlaylistItem> {
        if self.items.is_empty() {
            return None;
        }

        if self.is_shuffle && self.items.len() > 1 {
            return self.next_shuffled();
        }

        let wraps = self.repeat_mode != RepeatMode::Off;
        let next_idx = match self.current_index {
            Some(cur) => {
                if cur + 1 < self.items.len() {
                    cur + 1
                } else if wraps {
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

        if self.is_shuffle {
            // Walk back through what was actually played.
            if let Some(prev_idx) = self.shuffle_history.pop() {
                if prev_idx < self.items.len() {
                    self.current_index = Some(prev_idx);
                    return self.current_item();
                }
            }
            return self.current_item();
        }

        let wraps = self.repeat_mode != RepeatMode::Off;
        let prev_idx = match self.current_index {
            Some(cur) => {
                if cur > 0 {
                    cur - 1
                } else if wraps {
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

    fn next_shuffled(&mut self) -> Option<&PlaylistItem> {
        let len = self.items.len();
        if let Some(cur) = self.current_index {
            self.shuffle_history.push(cur);
        }
        // Prefer tracks not played in this shuffle cycle.
        let mut candidates: Vec<usize> = (0..len)
            .filter(|i| Some(*i) != self.current_index && !self.shuffle_history.contains(i))
            .collect();
        if candidates.is_empty() {
            if self.repeat_mode == RepeatMode::Off {
                self.shuffle_history.pop();
                return None;
            }
            // New cycle: everything except the current item is eligible again.
            self.shuffle_history.clear();
            if let Some(cur) = self.current_index {
                self.shuffle_history.push(cur);
            }
            candidates = (0..len).filter(|i| Some(*i) != self.current_index).collect();
        }
        let pick = candidates[(self.next_random() % candidates.len() as u64) as usize];
        self.current_index = Some(pick);
        self.current_item()
    }

    /// xorshift64*, seeded lazily from the clock — good enough for shuffle order.
    fn next_random(&mut self) -> u64 {
        if self.rng_state == 0 {
            self.rng_state = std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap_or_default()
                .as_nanos() as u64
                | 1;
        }
        let mut x = self.rng_state;
        x ^= x >> 12;
        x ^= x << 25;
        x ^= x >> 27;
        self.rng_state = x;
        x.wrapping_mul(0x2545_F491_4F6C_DD1D)
    }

    /// Expands a mix of files, folders, playlists and URLs into playable media (folders sorted
    /// naturally, playlists inlined) plus any subtitle files that were given directly.
    pub fn collect_media(inputs: &[PathBuf]) -> CollectedMedia {
        let mut out = CollectedMedia::default();
        for input in inputs {
            if is_url(input) {
                out.media.push(input.clone());
            } else if input.is_dir() {
                out.media.extend(Self::media_in_folder(input));
            } else {
                let ext = input.extension().and_then(|e| e.to_str()).unwrap_or("").to_lowercase();
                if Self::is_playlist_extension(&ext) && Self::looks_like_playlist(input) {
                    if let Ok(entries) = Self::load_playlist_file(input) {
                        out.media.extend(entries);
                    }
                } else if Self::is_supported_subtitle_extension(&ext) {
                    out.subtitles.push(input.clone());
                } else if Self::is_supported_media_extension(&ext) || Self::is_playlist_extension(&ext) {
                    out.media.push(input.clone());
                }
            }
        }
        out
    }

    /// Supported media files directly inside `folder`, in natural filename order.
    pub fn media_in_folder(folder: &Path) -> Vec<PathBuf> {
        let mut files: Vec<PathBuf> = std::fs::read_dir(folder)
            .map(|entries| {
                entries
                    .flatten()
                    .map(|e| e.path())
                    .filter(|p| {
                        p.is_file()
                            && p.extension()
                                .and_then(|e| e.to_str())
                                .is_some_and(Self::is_supported_media_extension)
                    })
                    .collect()
            })
            .unwrap_or_default();
        files.sort_by(|a, b| {
            let an = a.file_name().map(|n| n.to_string_lossy().to_string()).unwrap_or_default();
            let bn = b.file_name().map(|n| n.to_string_lossy().to_string()).unwrap_or_default();
            natural_cmp(&an, &bn)
        });
        files
    }

    pub fn is_playlist_extension(ext: &str) -> bool {
        matches!(ext.to_lowercase().as_str(), "m3u" | "m3u8" | "pls")
    }

    /// A local `.m3u8` may be an HLS stream manifest rather than a list of files; those are
    /// played as media instead of being expanded.
    fn looks_like_playlist(path: &Path) -> bool {
        let is_m3u8 = path.extension().and_then(|e| e.to_str()).is_some_and(|e| e.eq_ignore_ascii_case("m3u8"));
        if !is_m3u8 {
            return true;
        }
        match read_text_capped(path) {
            Ok(text) => !text.contains("#EXT-X-"),
            Err(_) => true,
        }
    }

    /// Reads an M3U/M3U8 or PLS playlist. Relative entries resolve against the playlist's folder.
    pub fn load_playlist_file(path: &Path) -> Result<Vec<PathBuf>, String> {
        let text = read_text_capped(path)?;
        let base = path.parent().unwrap_or(Path::new("."));
        let is_pls = path.extension().and_then(|e| e.to_str()).is_some_and(|e| e.eq_ignore_ascii_case("pls"));
        Ok(parse_playlist(&text, base, is_pls))
    }

    /// Writes the playlist as extended M3U (UTF-8).
    pub fn save_m3u(&self, path: &Path) -> Result<(), String> {
        let mut out = String::from("#EXTM3U\n");
        for item in &self.items {
            let title = item.title.replace(['\r', '\n'], " ");
            out.push_str(&format!("#EXTINF:-1,{title}\n{}\n", item.path.to_string_lossy()));
        }
        std::fs::write(path, out).map_err(|e| format!("Failed to save playlist: {e}"))
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
                | "m2ts"
                | "mts"
                | "m4v"
                | "mpg"
                | "mpeg"
                | "vob"
                | "ogv"
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
                | "aiff"
                | "ape"
                | "wv"
                | "mka"
        )
    }

    pub fn is_supported_subtitle_extension(ext: &str) -> bool {
        SUBTITLE_EXTENSIONS.contains(&ext.to_lowercase().as_str())
    }
}

/// Playlist files above this size are refused (a real playlist is a few KB).
const MAX_PLAYLIST_BYTES: u64 = 4 * 1024 * 1024;

fn read_text_capped(path: &Path) -> Result<String, String> {
    let size = std::fs::metadata(path).map_err(|e| e.to_string())?.len();
    if size > MAX_PLAYLIST_BYTES {
        return Err("Playlist file is too large".into());
    }
    let bytes = std::fs::read(path).map_err(|e| e.to_string())?;
    Ok(crate::media::subtitle::decode_subtitle_bytes(&bytes))
}

/// Parses M3U (one entry per non-comment line) or PLS (`FileN=` keys) text.
pub fn parse_playlist(text: &str, base: &Path, is_pls: bool) -> Vec<PathBuf> {
    text.lines()
        .map(str::trim)
        .filter_map(|line| {
            if is_pls {
                let (key, value) = line.split_once('=')?;
                key.to_ascii_lowercase().starts_with("file").then(|| value.trim())
            } else if line.is_empty() || line.starts_with('#') {
                None
            } else {
                Some(line)
            }
        })
        .filter(|entry| !entry.is_empty())
        .map(|entry| {
            let entry = entry.strip_prefix("file:///").map(|p| {
                // file:///C:/x on Windows, file:///home/x elsewhere
                if cfg!(windows) { p.to_string() } else { format!("/{p}") }
            }).unwrap_or_else(|| entry.to_string());
            let p = PathBuf::from(&entry);
            if is_url(&p) || p.is_absolute() {
                p
            } else {
                base.join(p)
            }
        })
        .collect()
}

/// Compares strings so that embedded numbers sort numerically ("Ep 2" < "Ep 10").
pub fn natural_cmp(a: &str, b: &str) -> Ordering {
    let (mut ai, mut bi) = (a.chars().peekable(), b.chars().peekable());
    loop {
        match (ai.peek().copied(), bi.peek().copied()) {
            (None, None) => return Ordering::Equal,
            (None, Some(_)) => return Ordering::Less,
            (Some(_), None) => return Ordering::Greater,
            (Some(x), Some(y)) if x.is_ascii_digit() && y.is_ascii_digit() => {
                let take_num = |it: &mut std::iter::Peekable<std::str::Chars>| {
                    let mut s = String::new();
                    while let Some(c) = it.peek().copied().filter(char::is_ascii_digit) {
                        s.push(c);
                        it.next();
                    }
                    s
                };
                let (na, nb) = (take_num(&mut ai), take_num(&mut bi));
                let (ta, tb) = (na.trim_start_matches('0'), nb.trim_start_matches('0'));
                let ord = ta.len().cmp(&tb.len()).then_with(|| ta.cmp(tb)).then_with(|| na.len().cmp(&nb.len()));
                if ord != Ordering::Equal {
                    return ord;
                }
            }
            (Some(x), Some(y)) => {
                let ord = x.to_lowercase().cmp(y.to_lowercase());
                if ord != Ordering::Equal {
                    return ord;
                }
                ai.next();
                bi.next();
            }
        }
    }
}
