//! Thumbnails and metadata for the library. Everything slow (ffprobe, ffmpeg, image decoding)
//! happens on background threads and is cached on disk; the UI only ever asks "what do you have
//! for this path right now?" and paints a generated placeholder until the answer arrives.

use super::icons::Icon;
use super::theme::Theme;
use crate::library::{format_duration_short, MediaKind};
use crate::media::probe::MediaInfo;
use crate::media::{create_hidden_command, ffmpeg_input_args, is_url, output_with_timeout};
use crossbeam_channel::{Receiver, Sender};
use eframe::egui::{
    self, ecolor::Hsva, epaint::RectShape, Color32, ColorImage, CornerRadius, Painter, Pos2, Rect, TextureHandle,
    TextureOptions, Vec2,
};
use parking_lot::{Condvar, Mutex};
use std::collections::hash_map::DefaultHasher;
use std::collections::{HashMap, HashSet};
use std::hash::{Hash, Hasher};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::Duration;

/// Width of stored thumbnails: sharp on cards and acceptable behind the hero banner.
const THUMB_WIDTH: u32 = 640;
/// Textures kept in GPU memory; far more than a screen can show at once.
const MAX_TEXTURES: usize = 96;
/// Decoded images uploaded per frame, so a burst never causes a visible hitch.
const MAX_UPLOADS_PER_FRAME: usize = 4;
const WORKER_THREADS: usize = 2;
const THUMB_TIMEOUT: Duration = Duration::from_secs(20);
const MAX_CACHE_FILES: usize = 6000;
const PALETTE_SIZE: usize = 10;

/// What the library knows about a file besides its name.
#[derive(Debug, Clone, Default, serde::Serialize, serde::Deserialize)]
#[serde(default)]
pub struct ArtMeta {
    pub duration_secs: f64,
    pub height: u32,
    pub video_codec: Option<String>,
    pub audio_only: bool,
    pub title: Option<String>,
    pub artist: Option<String>,
    /// ffmpeg ran and found no picture; don't try again.
    pub no_art: bool,
}

impl ArtMeta {
    fn from_probe(info: &MediaInfo) -> Self {
        let video = info.primary_video_stream();
        // Embedded album art counts as a "video" stream, so judge by the codec's frame count.
        let is_cover_art = video.is_some_and(|v| matches!(v.codec_name.as_deref(), Some("mjpeg" | "png" | "bmp")));
        let audio_only = video.is_none() || (is_cover_art && !info.audio_streams().is_empty());
        Self {
            duration_secs: info.duration_seconds(),
            height: if audio_only { 0 } else { info.video_dimensions().map_or(0, |(_, h)| h) },
            video_codec: if audio_only { None } else { video.and_then(|v| v.codec_name.clone()) },
            audio_only,
            title: info.title().filter(|t| !t.trim().is_empty()),
            artist: info.artist().filter(|a| !a.trim().is_empty()),
            no_art: false,
        }
    }

    /// `1080p`, `4K`, ... or `None` for audio.
    pub fn resolution_label(&self) -> Option<String> {
        match self.height {
            0 => None,
            h if h >= 2000 => Some("4K".to_string()),
            h => Some(format!("{h}p")),
        }
    }

    /// `1h 42m · 1080p` style summary.
    pub fn summary(&self) -> String {
        let mut parts = Vec::new();
        if self.duration_secs > 0.0 {
            parts.push(format_duration_short(self.duration_secs));
        }
        if let Some(res) = self.resolution_label() {
            parts.push(res);
        }
        parts.join(" · ")
    }
}

struct Loaded {
    path: PathBuf,
    meta: Option<ArtMeta>,
    image: Option<ColorImage>,
}

#[derive(Default)]
struct Queue {
    /// A stack: what was requested last is what is on screen now.
    pending: Mutex<Vec<PathBuf>>,
    wake: Condvar,
    shutdown: AtomicBool,
}

struct Slot {
    texture: TextureHandle,
    last_used: u64,
}

/// Lazily loads artwork. Call [`Self::begin_frame`] once per frame before painting.
pub struct ArtworkCache {
    queue: Arc<Queue>,
    tx: Sender<Loaded>,
    rx: Receiver<Loaded>,
    started: bool,
    frame: u64,
    slots: HashMap<PathBuf, Slot>,
    meta: HashMap<PathBuf, ArtMeta>,
    /// Paths requested from the workers or known to have no picture.
    requested: HashSet<PathBuf>,
    palette: Vec<TextureHandle>,
    scrim: Option<TextureHandle>,
}

impl Default for ArtworkCache {
    fn default() -> Self {
        let (tx, rx) = crossbeam_channel::unbounded();
        Self {
            queue: Arc::new(Queue::default()),
            tx,
            rx,
            started: false,
            frame: 0,
            slots: HashMap::new(),
            meta: HashMap::new(),
            requested: HashSet::new(),
            palette: Vec::new(),
            scrim: None,
        }
    }
}

impl Drop for ArtworkCache {
    fn drop(&mut self) {
        self.queue.shutdown.store(true, Ordering::SeqCst);
        self.queue.wake.notify_all();
    }
}

impl ArtworkCache {
    pub fn begin_frame(&mut self, ctx: &egui::Context) {
        if !self.started {
            self.started = true;
            self.start(ctx);
        }
        self.frame += 1;

        for _ in 0..MAX_UPLOADS_PER_FRAME {
            let Ok(loaded) = self.rx.try_recv() else { break };
            if let Some(meta) = loaded.meta {
                self.meta.insert(loaded.path.clone(), meta);
            }
            if let Some(image) = loaded.image {
                let name = format!("art:{}", loaded.path.display());
                let options = TextureOptions { mipmap_mode: Some(egui::TextureFilter::Linear), ..TextureOptions::LINEAR };
                let texture = ctx.load_texture(name, image, options);
                self.slots.insert(loaded.path, Slot { texture, last_used: self.frame });
            }
        }
        if !self.rx.is_empty() {
            ctx.request_repaint();
        }

        if self.slots.len() > MAX_TEXTURES {
            let excess = self.slots.len() - MAX_TEXTURES;
            let mut by_age: Vec<(u64, PathBuf)> = self.slots.iter().map(|(p, s)| (s.last_used, p.clone())).collect();
            by_age.sort_by_key(|(age, _)| *age);
            for (_, path) in by_age.into_iter().take(excess) {
                self.slots.remove(&path);
                // Scrolling back reloads it from the disk cache.
                self.requested.remove(&path);
            }
        }
    }

    fn start(&mut self, ctx: &egui::Context) {
        self.palette = (0..PALETTE_SIZE).map(|i| ctx.load_texture(format!("art_palette_{i}"), palette_image(i), TextureOptions::LINEAR)).collect();
        self.scrim = Some(ctx.load_texture("art_scrim", scrim_image(), TextureOptions::LINEAR));

        let Some(dir) = crate::persistence::config_dir().map(|d| d.join("thumbnails")) else { return };
        if std::fs::create_dir_all(&dir).is_err() {
            return;
        }
        for _ in 0..WORKER_THREADS {
            let (queue, tx, ctx, dir) = (Arc::clone(&self.queue), self.tx.clone(), ctx.clone(), dir.clone());
            std::thread::spawn(move || worker(queue, tx, ctx, dir));
        }
        std::thread::spawn(move || trim_cache(&dir));
    }

    fn request(&mut self, path: &Path) {
        if is_url(path) || self.requested.contains(path) {
            return; // never touch the network just to decorate a card
        }
        self.requested.insert(path.to_path_buf());
        self.queue.pending.lock().push(path.to_path_buf());
        self.queue.wake.notify_one();
    }

    /// The picture for `path` if it is ready; asks for it otherwise.
    pub fn texture(&mut self, path: &Path) -> Option<TextureHandle> {
        let frame = self.frame;
        if let Some(slot) = self.slots.get_mut(path) {
            slot.last_used = frame;
            return Some(slot.texture.clone());
        }
        self.request(path);
        None
    }

    /// Metadata for `path` if it is known yet.
    pub fn meta(&self, path: &Path) -> Option<&ArtMeta> {
        self.meta.get(path)
    }

    /// Fills `rect` with the artwork of `path`, cropped to fit, or a generated placeholder.
    pub fn paint(&mut self, painter: &Painter, rect: Rect, radius: CornerRadius, path: &Path, title: &str, kind: MediaKind) {
        match self.texture(path) {
            Some(tex) => paint_cover(painter, rect, radius, &tex, Color32::WHITE),
            None => self.paint_placeholder(painter, rect, radius, title, kind),
        }
    }

    /// A gradient chosen by the title, so a placeholder stays the same between runs.
    pub fn paint_placeholder(&self, painter: &Painter, rect: Rect, radius: CornerRadius, title: &str, kind: MediaKind) {
        let mut h = DefaultHasher::new();
        title.hash(&mut h);
        if let Some(tex) = self.palette.get((h.finish() % PALETTE_SIZE as u64) as usize) {
            painter.add(RectShape::filled(rect, radius, Color32::WHITE).with_texture(tex.id(), full_uv()));
        } else {
            painter.rect_filled(rect, radius, Theme::SURFACE);
        }
        let icon = match kind {
            MediaKind::Video => Icon::Film,
            MediaKind::Audio => Icon::Music,
            MediaKind::Stream => Icon::Globe,
        };
        let size = (rect.height() * 0.3).min(72.0);
        icon.paint(painter, Rect::from_center_size(rect.center(), Vec2::splat(size)), Theme::white(70));
    }

    /// Darkens toward the bottom of `rect`, respecting rounded corners.
    pub fn paint_scrim(&self, painter: &Painter, rect: Rect, radius: CornerRadius, alpha: u8) {
        if let Some(tex) = &self.scrim {
            painter.add(RectShape::filled(rect, radius, Color32::from_black_alpha(alpha)).with_texture(tex.id(), full_uv()));
        }
    }
}

fn full_uv() -> Rect {
    Rect::from_min_max(Pos2::ZERO, Pos2::new(1.0, 1.0))
}

/// Paints `tex` covering `rect`, cropping whichever dimension overflows and keeping it centered.
pub fn paint_cover(painter: &Painter, rect: Rect, radius: CornerRadius, tex: &TextureHandle, tint: Color32) {
    let [w, h] = tex.size();
    let (img_aspect, rect_aspect) = (w as f32 / h.max(1) as f32, rect.width() / rect.height().max(1.0));
    let uv = if img_aspect > rect_aspect {
        let span = rect_aspect / img_aspect;
        Rect::from_min_max(Pos2::new(0.5 - span / 2.0, 0.0), Pos2::new(0.5 + span / 2.0, 1.0))
    } else {
        let span = img_aspect / rect_aspect;
        Rect::from_min_max(Pos2::new(0.0, 0.5 - span / 2.0), Pos2::new(1.0, 0.5 + span / 2.0))
    };
    painter.add(RectShape::filled(rect, radius, tint).with_texture(tex.id(), uv));
}

fn palette_image(i: usize) -> ColorImage {
    let (w, h) = (96usize, 54usize);
    // Hues spread around the wheel, offset so the first ones sit near the brand's warm tones.
    let hue = (0.02 + i as f32 / PALETTE_SIZE as f32) % 1.0;
    let hue2 = (hue + 0.07) % 1.0;
    let mut pixels = Vec::with_capacity(w * h);
    for y in 0..h {
        for x in 0..w {
            let t = (x as f32 / (w - 1) as f32 * 0.6 + y as f32 / (h - 1) as f32 * 0.4).clamp(0.0, 1.0);
            let a: Color32 = Hsva::new(hue, 0.62, 0.42, 1.0).into();
            let b: Color32 = Hsva::new(hue2, 0.55, 0.11, 1.0).into();
            pixels.push(super::icons::lerp_color(a, b, t));
        }
    }
    ColorImage { size: [w, h], pixels, source_size: Vec2::new(w as f32, h as f32) }
}

fn scrim_image() -> ColorImage {
    let h = 64usize;
    let pixels = (0..h)
        .map(|y| {
            let t = y as f32 / (h - 1) as f32;
            Color32::from_white_alpha((t * t * (3.0 - 2.0 * t) * 255.0) as u8)
        })
        .collect();
    ColorImage { size: [1, h], pixels, source_size: Vec2::new(1.0, h as f32) }
}

fn worker(queue: Arc<Queue>, tx: Sender<Loaded>, ctx: egui::Context, dir: PathBuf) {
    loop {
        let path = {
            let mut pending = queue.pending.lock();
            loop {
                if queue.shutdown.load(Ordering::SeqCst) {
                    return;
                }
                if let Some(p) = pending.pop() {
                    break p;
                }
                queue.wake.wait(&mut pending);
            }
        };
        let loaded = load(&path, &dir);
        if tx.send(loaded).is_err() {
            return;
        }
        ctx.request_repaint();
    }
}

/// Cache key: the file's identity, so a replaced file gets a fresh thumbnail.
fn cache_key(path: &Path) -> Option<String> {
    let meta = std::fs::metadata(path).ok()?;
    let mut h = DefaultHasher::new();
    path.hash(&mut h);
    meta.len().hash(&mut h);
    meta.modified().ok()?.hash(&mut h);
    Some(format!("{:016x}", h.finish()))
}

fn load(path: &Path, dir: &Path) -> Loaded {
    let mut out = Loaded { path: path.to_path_buf(), meta: None, image: None };
    let Some(key) = cache_key(path) else { return out };
    let meta_path = dir.join(format!("{key}.json"));
    let thumb_path = dir.join(format!("{key}.jpg"));

    let mut meta: Option<ArtMeta> = std::fs::read_to_string(&meta_path)
        .ok()
        .and_then(|s| serde_json::from_str(&s).ok());
    if meta.is_none() {
        // A failed probe (e.g. ffmpeg not installed yet) is not remembered, so it retries later.
        meta = MediaInfo::probe(path).ok().map(|info| ArtMeta::from_probe(&info));
    }
    let Some(mut meta) = meta else { return out };

    // A picture next to the file wins over anything extracted from it.
    let local = sidecar_image(path, meta.audio_only);
    if let Some(img) = local.as_deref().and_then(load_image) {
        out.image = Some(img);
    } else if let Some(img) = load_image(&thumb_path) {
        out.image = Some(img);
    } else if !meta.no_art {
        match extract_thumbnail(path, &thumb_path, &meta) {
            Ok(true) => out.image = load_image(&thumb_path),
            Ok(false) => meta.no_art = true,
            Err(_) => {} // ffmpeg couldn't run at all: try again next launch
        }
    }

    if let Ok(json) = serde_json::to_string(&meta) {
        let _ = std::fs::write(&meta_path, json);
    }
    out.meta = Some(meta);
    out
}

fn sidecar_image(path: &Path, audio_only: bool) -> Option<PathBuf> {
    const EXTS: [&str; 3] = ["jpg", "jpeg", "png"];
    let parent = path.parent()?;
    if let Some(stem) = path.file_stem() {
        for ext in EXTS {
            let candidate = parent.join(stem).with_extension(ext);
            if candidate.is_file() {
                return Some(candidate);
            }
        }
    }
    // Folder-wide art describes an album; on videos it would label every file the same.
    if audio_only {
        for name in ["cover", "folder", "album", "front"] {
            for ext in EXTS {
                let candidate = parent.join(format!("{name}.{ext}"));
                if candidate.is_file() {
                    return Some(candidate);
                }
            }
        }
    }
    None
}

fn load_image(path: &Path) -> Option<ColorImage> {
    let bytes = std::fs::read(path).ok()?;
    let img = image::load_from_memory(&bytes).ok()?;
    let img = if img.width() > THUMB_WIDTH { img.thumbnail(THUMB_WIDTH, THUMB_WIDTH) } else { img };
    let rgba = img.to_rgba8();
    let size = [rgba.width() as usize, rgba.height() as usize];
    Some(ColorImage::from_rgba_unmultiplied(size, &rgba.into_raw()))
}

/// Writes a JPEG thumbnail. `Ok(false)` means ffmpeg ran but the file has no usable picture.
fn extract_thumbnail(path: &Path, out: &Path, meta: &ArtMeta) -> Result<bool, String> {
    let mut cmd = create_hidden_command("ffmpeg");
    cmd.args(["-hide_banner", "-loglevel", "error", "-nostdin"]);
    let filter = if meta.audio_only {
        format!("scale={THUMB_WIDTH}:-2")
    } else {
        // Skip the usual black opening and let ffmpeg pick a representative frame nearby.
        let at = (meta.duration_secs * 0.15).min(300.0);
        if at >= 1.0 {
            cmd.args(["-ss", &format!("{at:.2}")]);
        }
        format!("thumbnail=24,scale={THUMB_WIDTH}:-2")
    };
    cmd.args(ffmpeg_input_args(path));
    cmd.args(["-map", "0:v:0", "-an", "-sn", "-frames:v", "1", "-vf", &filter, "-q:v", "4", "-y"]);
    let tmp = out.with_extension("tmp.jpg");
    cmd.arg(&tmp);

    let output = output_with_timeout(cmd, THUMB_TIMEOUT)?;
    let written = output.status.success() && std::fs::metadata(&tmp).is_ok_and(|m| m.len() > 0);
    if written {
        std::fs::rename(&tmp, out).map_err(|e| e.to_string())?;
    } else {
        let _ = std::fs::remove_file(&tmp);
    }
    Ok(written)
}

/// Keeps the thumbnail folder from growing forever by dropping the oldest files.
fn trim_cache(dir: &Path) {
    let Ok(read) = std::fs::read_dir(dir) else { return };
    let mut files: Vec<(std::time::SystemTime, PathBuf)> = read
        .flatten()
        .filter_map(|e| Some((e.metadata().ok()?.modified().ok()?, e.path())))
        .collect();
    if files.len() <= MAX_CACHE_FILES {
        return;
    }
    files.sort_by_key(|(time, _)| *time);
    let excess = files.len() - MAX_CACHE_FILES;
    for (_, path) in files.into_iter().take(excess) {
        let _ = std::fs::remove_file(path);
    }
}
