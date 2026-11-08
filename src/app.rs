use crate::ab_loop::AbLoop;
use crate::library::{display_title, library_signature, Library};
use crate::media::decoder::HwAccelMode;
use crate::media::{is_url, MediaPlayer};
use crate::persistence::SessionState;
use crate::playlist::{Playlist, RepeatMode};
use crate::ui::about_dialog::AboutDialog;
use crate::ui::artwork::ArtworkCache;
use crate::ui::browse::{self, BrowseAction, BrowseCx, BrowseState};
use crate::ui::codec_dialog::{CodecDialog, CodecDialogState};
use crate::ui::canvas::{AspectRatioMode, AudioStage, VideoCanvas};
use crate::ui::controls::{ControlsCtx, ControlsView, BOTTOM_CONTROLS_H};
use crate::ui::equalizer_dialog::EqualizerDialog;
use crate::ui::info_dialog::InfoDialog;
use crate::ui::jump_dialog::JumpDialog;

use crate::ui::menu_bar::{media_dialog, MenuBarView, MenuCtx, OpenRequest};
use crate::ui::mini_player::{self, MiniPlayerCx, MINI_PLAYER_HEIGHT};
use crate::ui::playlist_view::PlaylistView;
use crate::ui::preferences_dialog::{PreferencesDialog, PreferencesState};
use crate::ui::shortcuts_dialog::ShortcutsDialog;
use crate::ui::stream_dialog::StreamDialog;
use crate::ui::theme::Theme;

use eframe::egui::{self, Align2, Key, Popup, PopupCloseBehavior, Pos2, Rect, Response, Sense, Stroke, Ui, UiBuilder, Vec2};
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

/// How often session state (playlist, resume positions, ...) is flushed to disk.
const SESSION_SAVE_INTERVAL: Duration = Duration::from_secs(10);
/// A pipeline that "finishes" this quickly after starting failed to play rather than ended.
const MIN_PLAYBACK_BEFORE_ADVANCE: Duration = Duration::from_millis(500);

/// The two top-level screens.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum View {
    /// The library: home page, grids and search.
    #[default]
    Browse,
    /// Fullscreen-style playback.
    Player,
}

pub struct AirMediaApp {
    pub view: View,
    pub player: MediaPlayer,
    pub playlist: Playlist,
    pub canvas: VideoCanvas,
    pub controls: ControlsView,
    pub show_playlist: bool,
    pub show_info: bool,
    pub show_equalizer: bool,
    pub show_stream: bool,
    pub show_preferences: bool,
    pub show_about: bool,
    pub show_codecs: bool,
    pub show_shortcuts: bool,
    pub show_jump: bool,
    pub jump_input: String,
    pub ab_loop: AbLoop,
    pub codec_state: CodecDialogState,
    pub preferences_state: PreferencesState,
    /// Recent files, playlist, equalizer and resume positions persisted across runs.
    pub session: SessionState,
    pub stream_url_input: String,
    pub is_fullscreen: bool,
    pub last_mouse_activity: Instant,
    pub last_pointer_pos: Option<Pos2>,
    /// Message, when it was shown, and how long it stays.
    pub toast: Option<(String, Instant, f32)>,
    window_title: String,
    /// File whose position is being tracked for resume, with its last known position/duration.
    tracked: Option<(PathBuf, f64, f64)>,
    last_session_save: Instant,
    last_session_json: String,
    /// Window level last sent to the OS, so the command is only sent on change.
    applied_always_on_top: Option<bool>,
    logo_texture: Option<egui::TextureHandle>,
    artwork: ArtworkCache,
    library: Library,
    library_signature: Option<u64>,
    /// Per-run memo of file existence: a dead network drive must not stall every rebuild.
    known_paths: HashMap<PathBuf, bool>,
    browse: BrowseState,
    /// Work requested by the UI, run at the end of the frame.
    actions: Vec<BrowseAction>,
    /// Library item whose details overlay is open.
    details: Option<PathBuf>,
    controls_hovered: bool,
    /// Whether the controls were showing when the current press began (touch toggling).
    controls_up_at_press: bool,
}

impl AirMediaApp {
    pub fn new() -> Self {
        let codec_state = CodecDialogState::default();
        let show_codecs = !codec_state.status.is_ready();
        let preferences_state = PreferencesState::load();
        let session = SessionState::load();

        let mut player = MediaPlayer::new();
        player.hw_accel = preferences_state.hw_accel;
        player.set_volume(preferences_state.default_volume);
        player.audio_filter = session.equalizer.audio_filter();
        player.video_filter = session.equalizer.video_filter();

        // Restore the previous playlist (queued, not auto-played).
        let mut playlist = Playlist::new();
        for path in &session.playlist {
            playlist.add(path);
        }
        playlist.current_index = session.playlist_index.filter(|i| *i < playlist.items.len());
        playlist.repeat_mode = session.repeat_mode;
        playlist.is_shuffle = session.shuffle;

        let last_session_json = session.to_json();
        Self {
            view: View::Browse,
            player,
            playlist,
            canvas: VideoCanvas::new(),
            controls: ControlsView::new(),
            show_playlist: false,
            show_info: false,
            show_equalizer: false,
            show_stream: false,
            show_preferences: false,
            show_about: false,
            show_codecs,
            show_shortcuts: false,
            show_jump: false,
            jump_input: String::new(),
            ab_loop: AbLoop::default(),
            codec_state,
            preferences_state,
            session,
            stream_url_input: String::new(),
            is_fullscreen: false,
            last_mouse_activity: Instant::now(),
            last_pointer_pos: None,
            toast: None,
            window_title: String::new(),
            tracked: None,
            last_session_save: Instant::now(),
            last_session_json,
            applied_always_on_top: None,
            logo_texture: None,
            artwork: ArtworkCache::default(),
            library: Library::default(),
            library_signature: None,
            known_paths: HashMap::new(),
            browse: BrowseState::default(),
            actions: Vec::new(),
            details: None,
            controls_hovered: false,
            controls_up_at_press: false,
        }
    }

    pub fn set_toast(&mut self, msg: impl Into<String>) {
        self.toast = Some((msg.into(), Instant::now(), 1.4));
    }

    /// A toast that stays long enough to read an error message.
    pub fn set_error_toast(&mut self, msg: impl Into<String>) {
        self.toast = Some((msg.into(), Instant::now(), 4.0));
    }

    /// Expands files, folders, playlists and URLs, appends the media to the playlist and
    /// optionally starts the first new item. Subtitle files attach to the current media.
    pub fn open_paths(&mut self, inputs: Vec<PathBuf>, play: bool) {
        if inputs.is_empty() {
            return;
        }
        let collected = Playlist::collect_media(&inputs);
        // Reuse entries already in the playlist (e.g. restored from the last session) instead
        // of piling up duplicates every time the same file is opened again.
        let mut first_target = None;
        for path in &collected.media {
            let idx = match self.playlist.items.iter().position(|i| &i.path == path) {
                Some(existing) => existing,
                None => {
                    self.playlist.add(path);
                    self.playlist.items.len() - 1
                }
            };
            first_target.get_or_insert(idx);
        }
        let added = collected.media.len();

        if let Some(first) = first_target.filter(|_| play) {
            self.play_index(first);
            if added > 1 && self.player.current_file.is_some() {
                self.set_toast(format!("Added {added} items to the playlist"));
            }
        } else if added > 0 {
            self.set_toast(if added == 1 { "Added to playlist".to_string() } else { format!("Added {added} items to the playlist") });
        }

        for sub in collected.subtitles {
            if self.player.current_file.is_none() {
                self.set_toast("Open a video before loading subtitles");
                break;
            }
            if self.player.load_external_subtitle(&sub).is_ok() {
                self.set_toast("Subtitles loaded");
            }
        }

        if added == 0 && self.toast.is_none() {
            self.set_toast("No playable media found");
        }
    }

    /// Loads playlist item `idx`. Errors surface through `player.take_error()`.
    fn play_index(&mut self, idx: usize) {
        let Some(path) = self.playlist.items.get(idx).map(|i| i.path.clone()) else { return };
        self.playlist.current_index = Some(idx);
        let _ = self.player.load_file(&path);
    }

    /// Records resume positions, applies a saved one when a new file starts, and keeps the
    /// recent-files list in sync — regardless of which UI path loaded the file.
    fn track_current_file(&mut self) {
        let current = self.player.current_file.clone();
        let changed = self.tracked.as_ref().map(|(p, _, _)| p) != current.as_ref();

        if changed {
            self.ab_loop.clear();
            self.canvas.clear();
            if current.is_some() {
                self.view = View::Player;
            }
            if let Some((old, pos, dur)) = self.tracked.take() {
                if !is_url(&old) {
                    self.session.record_position(&old, pos, dur);
                }
            }
            if let Some(path) = &current {
                self.session.push_recent(path);
                if !is_url(path) {
                    if let Some(pos) = self.session.resume_position(path) {
                        if pos < self.player.duration_secs * crate::persistence::RESUME_MAX_FRACTION {
                            self.player.seek(pos);
                            self.set_toast(format!("Resumed at {}", format_clock(pos)));
                        }
                    }
                }
                self.tracked = Some((path.clone(), 0.0, self.player.duration_secs));
            }
        }

        if let Some((_, pos, dur)) = &mut self.tracked {
            *pos = self.player.current_time_secs();
            *dur = self.player.duration_secs;
        }
    }

    fn sync_session(&mut self) {
        if let Some((path, pos, dur)) = &self.tracked {
            if !is_url(path) {
                let (path, pos, dur) = (path.clone(), *pos, *dur);
                self.session.record_position(&path, pos, dur);
            }
        }
        self.session.playlist = self.playlist.items.iter().map(|i| i.path.clone()).collect();
        self.session.playlist_index = self.playlist.current_index;
        self.session.repeat_mode = self.playlist.repeat_mode;
        self.session.shuffle = self.playlist.is_shuffle;
    }

    /// Writes the session file if anything changed since the last write.
    pub fn save_session(&mut self) {
        self.sync_session();
        let json = self.session.to_json();
        if json != self.last_session_json {
            SessionState::save_json(&json);
            self.last_session_json = json;
        }
        self.last_session_save = Instant::now();
    }

    /// Moves on when the current item has played to the end.
    fn handle_end_of_media(&mut self) {
        if !self.player.is_playing || !self.player.is_finished() {
            return;
        }
        if self.player.playback_started_at.elapsed() < MIN_PLAYBACK_BEFORE_ADVANCE {
            // Ended immediately: the media can't be decoded. Don't spin through retries.
            self.close_media();
            self.set_error_toast("Playback failed: the media could not be decoded");
            return;
        }
        if let Some(a) = self.ab_loop.jump_target(self.player.duration_secs) {
            // B at (or past) the end: loop back instead of advancing.
            self.player.seek(a);
            return;
        }
        if let Some((path, _, dur)) = &mut self.tracked {
            // Played to the end: nothing to resume.
            let path = path.clone();
            let dur = *dur;
            self.session.record_position(&path, dur, dur);
            if let Some((_, pos, _)) = &mut self.tracked {
                *pos = dur;
            }
        }
        if self.playlist.repeat_mode == RepeatMode::One && self.playlist.current_item().is_some() {
            self.player.seek(0.0);
            return;
        }
        if let Some(next) = self.playlist.next_auto() {
            let (path, title) = (next.path.clone(), next.title.clone());
            if self.player.load_file(&path).is_ok() {
                self.set_toast(format!("Playing: {title}"));
            }
        } else {
            self.close_media();
        }
    }

    /// Stops playback and returns to the library. The position is saved by the next
    /// `track_current_file`, so a stopped item can be resumed from its card.
    fn close_media(&mut self) {
        self.player.unload();
        self.view = View::Browse;
    }

    /// A card or the banner asked to play `path`.
    fn play_from_library(&mut self, path: PathBuf) {
        if self.player.current_file.as_deref() == Some(path.as_path()) {
            self.view = View::Player;
            if !self.player.is_playing {
                self.player.resume();
            }
        } else {
            self.open_paths(vec![path], true);
        }
    }

    /// Rebuilds the library if the playlist or history changed.
    fn refresh_library(&mut self) {
        let resume = self.session.resume_paths();
        let signature = library_signature(&self.playlist, &self.session.recent_files, &resume);
        if self.library_signature == Some(signature) {
            return;
        }
        self.library_signature = Some(signature);
        let playlist: Vec<PathBuf> = self.playlist.items.iter().map(|i| i.path.clone()).collect();
        let known = &mut self.known_paths;
        let mut exists = |p: &Path| *known.entry(p.to_path_buf()).or_insert_with(|| p.exists());
        self.library = Library::build(&playlist, &self.session.recent_files, &resume, &mut exists);
    }

    fn run_actions(&mut self, ctx: &egui::Context) {
        for action in std::mem::take(&mut self.actions) {
            match action {
                BrowseAction::Play(path) => self.play_from_library(path),
                BrowseAction::PlayFromStart(path) => {
                    self.session.forget_position(&path);
                    if self.player.current_file.as_deref() == Some(path.as_path()) {
                        self.player.seek(0.0);
                        self.view = View::Player;
                    } else {
                        self.open_paths(vec![path], true);
                    }
                }
                BrowseAction::ShowDetails(path) => self.details = Some(path),
                BrowseAction::AddToPlaylist(path) => {
                    if !self.playlist.items.iter().any(|i| i.path == path) {
                        self.playlist.add(&path);
                        self.set_toast("Added to playlist");
                    }
                }
                BrowseAction::RemoveFromPlaylist(path) => {
                    if let Some(idx) = self.playlist.items.iter().position(|i| i.path == path) {
                        self.playlist.remove(idx);
                        self.set_toast("Removed from playlist");
                    }
                }
                BrowseAction::ForgetHistory(path) => {
                    self.session.forget(&path);
                    self.set_toast("Removed from history");
                }
                BrowseAction::OpenFile => self.open_file_dialog(),
                BrowseAction::OpenFiles => {
                    if let Some(files) = media_dialog().pick_files() {
                        self.open_paths(files, false);
                    }
                }
                BrowseAction::OpenFolder => {
                    if let Some(folder) = crate::ui::FileDialog::new().pick_folder() {
                        self.open_paths(vec![folder], false);
                    }
                }
                BrowseAction::OpenStream => self.show_stream = true,
                BrowseAction::TogglePlaylist => self.show_playlist = !self.show_playlist,
            }
        }
        // A file dialog may have blocked for a while; don't count that as idle time.
        self.last_mouse_activity = Instant::now();
        ctx.request_repaint();
    }

    /// Current media's `(title, subtitle)`, or `None` when nothing is loaded.
    fn now_labels(&self) -> Option<(String, String)> {
        let path = self.player.current_file.as_deref()?;
        let info = self.player.media_info.as_ref();
        let video = info.and_then(|m| m.video_dimensions());
        let audio_only = info.is_some_and(|m| m.primary_video_stream().is_none());
        if audio_only {
            let title = info.and_then(|m| m.title()).filter(|t| !t.trim().is_empty()).unwrap_or_else(|| display_title(path));
            let credit: Vec<String> = [info.and_then(|m| m.artist()), info.and_then(|m| m.album())]
                .into_iter()
                .flatten()
                .filter(|s| !s.trim().is_empty())
                .collect();
            Some((title, credit.join(" \u{00b7} ")))
        } else {
            let codec = info.and_then(|m| m.primary_video_stream()).and_then(|v| v.codec_name.as_deref());
            let mut tech = Vec::new();
            if let Some((_, h)) = video {
                tech.push(if h >= 2000 { "4K".to_string() } else { format!("{h}p") });
            }
            if let Some(c) = codec {
                tech.push(c.to_uppercase());
            }
            Some((display_title(path), tech.join(" \u{00b7} ")))
        }
    }

    fn play_next(&mut self) {
        if let Some(item) = self.playlist.next() {
            let (path, title) = (item.path.clone(), item.title.clone());
            if self.player.load_file(&path).is_ok() {
                self.set_toast(format!("Next: {title}"));
            }
        }
    }

    fn play_prev(&mut self) {
        if let Some(item) = self.playlist.prev() {
            let (path, title) = (item.path.clone(), item.title.clone());
            if self.player.load_file(&path).is_ok() {
                self.set_toast(format!("Prev: {title}"));
            }
        }
    }

    /// B: set A, then B, then clear.
    fn cycle_ab_loop(&mut self) {
        let t = self.player.current_time_secs();
        if self.ab_loop.is_active() {
            self.ab_loop.clear();
            self.set_toast("A-B loop cleared");
        } else if self.ab_loop.a.is_none() {
            self.ab_loop.set_a(t);
            self.set_toast(format!("Loop A: {}", format_clock(t)));
        } else if self.ab_loop.set_b(t) {
            let a = self.ab_loop.a.unwrap_or(0.0);
            self.set_toast(format!("Loop A-B: {} - {}", format_clock(a), format_clock(t)));
        } else {
            self.set_toast("Loop B must be after A");
        }
    }

    fn open_jump_dialog(&mut self) {
        self.jump_input = format_clock(self.player.current_time_secs());
        self.show_jump = true;
    }

    /// Hardware media keys (delivered while the window has focus).
    fn handle_media_keys(&mut self, ctx: &egui::Context) {
        let (play_pause, next, prev, stop) = ctx.input(|i| {
            (
                i.key_pressed(Key::MediaPlayPause),
                i.key_pressed(Key::MediaTrackNext),
                i.key_pressed(Key::MediaTrackPrevious),
                i.key_pressed(Key::MediaStop),
            )
        });
        let has_media = self.player.current_file.is_some();
        if play_pause {
            if has_media {
                self.player.toggle_play();
            } else if let Some(idx) = self.playlist.current_index.or((!self.playlist.items.is_empty()).then_some(0)) {
                self.play_index(idx);
            }
        }
        if stop && has_media {
            self.close_media();
        }
        if next {
            self.play_next();
        }
        if prev {
            self.play_prev();
        }
    }

    fn handle_shortcuts(&mut self, ctx: &egui::Context) {
        self.handle_media_keys(ctx);
        if ctx.egui_wants_keyboard_input() {
            return;
        }
        let input = ctx.input(|i| i.clone());

        if input.modifiers.ctrl && input.key_pressed(Key::Q) {
            ctx.send_viewport_cmd(egui::ViewportCommand::Close);
        }

        let has_media = self.player.current_file.is_some();

        // Fullscreen toggle: F or F11
        if (input.key_pressed(Key::F) && !input.modifiers.ctrl) || input.key_pressed(Key::F11) {
            self.toggle_fullscreen(ctx);
        }

        // Ctrl+F: Search the library
        if input.modifiers.ctrl && input.key_pressed(Key::F) {
            self.view = View::Browse;
            self.browse.open_search();
        }

        // Esc steps back one level at a time. Open dialogs handle it themselves.
        if input.key_pressed(Key::Escape) && !self.any_dialog_open() && !Popup::is_any_open(ctx) {
            if self.is_fullscreen {
                self.is_fullscreen = false;
                ctx.send_viewport_cmd(egui::ViewportCommand::Fullscreen(false));
            } else if self.show_playlist {
                self.show_playlist = false;
            } else if self.view == View::Player {
                self.view = View::Browse;
            } else if self.browse.search_open {
                self.browse.close_search();
            }
        }

        // Space: Play / Pause (or start the featured item from the library)
        if input.key_pressed(Key::Space) {
            if has_media {
                self.player.toggle_play();
            } else {
                let up_next = self.playlist.current_item().map(|i| i.path.clone());
                if let Some((idx, _)) = self.library.featured(None, up_next.as_deref()) {
                    self.actions.push(BrowseAction::Play(self.library.entries[idx].path.clone()));
                }
            }
        }

        // S: Stop
        if has_media && input.key_pressed(Key::S) && !input.modifiers.ctrl && !input.modifiers.shift {
            self.close_media();
        }

        // Shift+S: Take Screenshot
        if has_media && input.modifiers.shift && input.key_pressed(Key::S) {
            match self.player.take_screenshot(
                &self.preferences_state.screenshot_dir,
                self.preferences_state.screenshot_format,
            ) {
                Ok(path) => self.set_toast(format!("Screenshot saved: {}", path.display())),
                Err(e) => self.set_toast(format!("Screenshot failed: {e}")),
            }
        }

        // M: Mute toggle
        if input.key_pressed(Key::M) {
            self.player.toggle_mute();
            if self.player.is_muted() {
                self.set_toast("Muted");
            } else {
                let vol_pct = (self.player.get_volume() * 100.0) as u32;
                self.set_toast(format!("Volume {vol_pct}%"));
            }
        }

        // Arrow Left / Right: Seek
        let shift = input.modifiers.shift;
        let ctrl = input.modifiers.ctrl;
        let seek_delta = if ctrl {
            60.0
        } else if shift {
            15.0
        } else {
            5.0
        };

        if has_media && input.key_pressed(Key::ArrowLeft) {
            let cur = self.player.current_time_secs();
            self.player.seek(cur - seek_delta);
            self.set_toast(format!("− {seek_delta} s"));
        }
        if has_media && input.key_pressed(Key::ArrowRight) {
            let cur = self.player.current_time_secs();
            self.player.seek(cur + seek_delta);
            self.set_toast(format!("+ {seek_delta} s"));
        }

        // Arrow Up / Down: Volume
        if input.key_pressed(Key::ArrowUp) {
            let cur_vol = self.player.get_volume();
            let new_vol = (cur_vol + 0.05).min(1.5);
            self.player.set_volume(new_vol);
            let pct = (new_vol * 100.0) as u32;
            self.set_toast(format!("Volume {pct}%"));
        }
        if input.key_pressed(Key::ArrowDown) {
            let cur_vol = self.player.get_volume();
            let new_vol = (cur_vol - 0.05).max(0.0);
            self.player.set_volume(new_vol);
            let pct = (new_vol * 100.0) as u32;
            self.set_toast(format!("Volume {pct}%"));
        }

        // Frame Stepping
        if has_media && input.key_pressed(Key::Period) {
            self.player.step_frame(true);
            self.set_toast("Next frame");
        }
        if has_media && input.key_pressed(Key::Comma) {
            self.player.step_frame(false);
            self.set_toast("Previous frame");
        }

        // Speed: [ and ]
        if has_media && input.key_pressed(Key::OpenBracket) {
            let spd = (self.player.playback_speed - 0.25).max(0.25);
            self.player.set_speed(spd);
            self.set_toast(format!("Speed {spd}×"));
        }
        if has_media && input.key_pressed(Key::CloseBracket) {
            let spd = (self.player.playback_speed + 0.25).min(2.5);
            self.player.set_speed(spd);
            self.set_toast(format!("Speed {spd}×"));
        }
        if has_media && (input.key_pressed(Key::Backspace) || input.key_pressed(Key::Equals)) {
            self.player.set_speed(1.0);
            self.set_toast("Speed 1×");
        }

        // Playlist Next / Prev: N / P
        if input.key_pressed(Key::N) && !ctrl {
            self.play_next();
        }
        if input.key_pressed(Key::P) && !ctrl {
            self.play_prev();
        }

        // B: A-B loop (set A, set B, clear); Ctrl+G: jump to time
        if has_media && !ctrl && input.key_pressed(Key::B) {
            self.cycle_ab_loop();
        }
        if has_media && ctrl && input.key_pressed(Key::G) {
            self.open_jump_dialog();
        }

        // ? / F1: keyboard shortcut help
        if input.key_pressed(Key::Questionmark)
            || (input.modifiers.shift && input.key_pressed(Key::Slash))
            || input.key_pressed(Key::F1)
        {
            self.show_shortcuts = !self.show_shortcuts;
        }

        // Ctrl+T: always on top
        if ctrl && input.key_pressed(Key::T) {
            self.preferences_state.always_on_top = !self.preferences_state.always_on_top;
            self.set_toast(if self.preferences_state.always_on_top { "Always on top" } else { "Always on top off" });
        }

        // Ctrl+L or L: Toggle playlist
        if input.key_pressed(Key::L) {
            self.show_playlist = !self.show_playlist;
        }

        // Ctrl+I or I: Toggle Codec & Media info
        if input.key_pressed(Key::I) {
            self.show_info = !self.show_info;
        }

        // Ctrl+E: Toggle Equalizer
        if input.modifiers.ctrl && input.key_pressed(Key::E) {
            self.show_equalizer = !self.show_equalizer;
        }

        // Ctrl+N: Open Network Stream
        if input.modifiers.ctrl && input.key_pressed(Key::N) {
            self.show_stream = true;
        }

        // Ctrl+P: Preferences
        if input.modifiers.ctrl && input.key_pressed(Key::P) {
            self.show_preferences = !self.show_preferences;
        }

        // O / Ctrl+O: Open file
        if input.key_pressed(Key::O) {
            self.open_file_dialog();
        }

        // A: Cycle Aspect Ratio
        if !ctrl && input.key_pressed(Key::A) {
            let next_mode = match self.canvas.aspect_mode {
                AspectRatioMode::Fit => AspectRatioMode::Fill,
                AspectRatioMode::Fill => AspectRatioMode::Force16_9,
                AspectRatioMode::Force16_9 => AspectRatioMode::Force4_3,
                AspectRatioMode::Force4_3 => AspectRatioMode::Force21_9,
                AspectRatioMode::Force21_9 => AspectRatioMode::Original,
                AspectRatioMode::Original => AspectRatioMode::Fit,
                _ => AspectRatioMode::Fit,
            };
            self.canvas.aspect_mode = next_mode;
            self.set_toast(format!("Aspect: {}", next_mode.name()));
        }

        // H: Cycle Hardware Acceleration
        if !ctrl && input.key_pressed(Key::H) {
            let next_hw = match self.player.hw_accel {
                HwAccelMode::Auto => HwAccelMode::D3D11VA,
                HwAccelMode::D3D11VA => HwAccelMode::D3D12VA,
                HwAccelMode::D3D12VA => HwAccelMode::Cuda,
                HwAccelMode::Cuda => HwAccelMode::Vulkan,
                HwAccelMode::Vulkan => HwAccelMode::Software,
                HwAccelMode::Software => HwAccelMode::Auto,
                _ => HwAccelMode::Auto,
            };
            self.player.set_hw_accel(next_hw);
            self.set_toast(format!("Decoder: {}", next_hw.name()));
        }
    }

    fn open_file_dialog(&mut self) {
        if let Some(path) = media_dialog().pick_file() {
            self.open_paths(vec![path], true);
        }
    }

    fn handle_drag_and_drop(&mut self, ctx: &egui::Context) {
        let dropped: Vec<PathBuf> = ctx.input(|i| {
            i.raw
                .dropped_files
                .iter()
                .map(|f| f.path().to_path_buf())
                .filter(|p| !p.as_os_str().is_empty())
                .collect()
        });
        if !dropped.is_empty() {
            self.open_paths(dropped, true);
        }
    }

    fn any_dialog_open(&self) -> bool {
        self.show_info
            || self.show_equalizer
            || self.show_stream
            || self.show_preferences
            || self.show_about
            || self.show_codecs
            || self.show_shortcuts
            || self.show_jump
            || self.details.is_some()
    }

    fn adjust_volume_by_wheel(&mut self, ctx: &egui::Context) {
        if Popup::is_any_open(ctx) {
            return;
        }
        let scroll = ctx.input(|i| i.smooth_scroll_delta.y);
        if scroll.abs() > 2.0 {
            let delta = if scroll > 0.0 { 0.05 } else { -0.05 };
            let new_vol = (self.player.get_volume() + delta).clamp(0.0, 1.5);
            self.player.set_volume(new_vol);
            self.set_toast(format!("Volume {}%", (new_vol * 100.0).round() as u32));
        }
    }

    /// Opens the app menu under `button` and applies whatever was chosen in it.
    fn app_menu(&mut self, ctx: &egui::Context, button: &Response) {
        let mut toast = None;
        let mut toggle_fs = false;
        let mut open_request: Option<OpenRequest> = None;
        let mut open_jump = false;
        let mut stop_request = false;
        let mut always_on_top = self.preferences_state.always_on_top;
        Popup::menu(button)
            .close_behavior(PopupCloseBehavior::CloseOnClickOutside)
            .gap(6.0)
            .show(|ui| {
                MenuBarView::show(
                    ui,
                    &mut self.player,
                    &mut self.playlist,
                    MenuCtx {
                        recent_files: &mut self.session.recent_files,
                        open_request: &mut open_request,
                        aspect_mode: &mut self.canvas.aspect_mode,
                        show_playlist: &mut self.show_playlist,
                        show_info: &mut self.show_info,
                        show_equalizer: &mut self.show_equalizer,
                        show_stream: &mut self.show_stream,
                        show_preferences: &mut self.show_preferences,
                        show_about: &mut self.show_about,
                        show_codecs: &mut self.show_codecs,
                        show_shortcuts: &mut self.show_shortcuts,
                        open_jump: &mut open_jump,
                        stop_request: &mut stop_request,
                        ab_loop: &mut self.ab_loop,
                        always_on_top: &mut always_on_top,
                        toggle_fullscreen: &mut toggle_fs,
                        toast_msg: &mut toast,
                        prefs: &self.preferences_state,
                    },
                );
            });
        self.preferences_state.always_on_top = always_on_top;
        if let Some(t) = toast {
            self.set_toast(t);
        }
        if let Some(req) = open_request {
            self.open_paths(req.paths, req.play);
        }
        if toggle_fs {
            self.toggle_fullscreen(ctx);
        }
        if open_jump {
            self.open_jump_dialog();
        }
        if stop_request {
            self.close_media();
        }
    }

    /// The library, with the now-playing bar docked under it when something is loaded.
    fn browse_screen(&mut self, ui: &mut Ui, ctx: &egui::Context, rect: Rect, logo: &egui::TextureHandle, levels: [f32; 16]) {
        self.refresh_library();

        let labels = self.now_labels();
        let (page_rect, bar_rect) = if labels.is_some() {
            let split = rect.max.y - MINI_PLAYER_HEIGHT;
            (
                Rect::from_min_max(rect.min, Pos2::new(rect.max.x, split)),
                Some(Rect::from_min_max(Pos2::new(rect.min.x, split), rect.max)),
            )
        } else {
            (rect, None)
        };

        let current = self.player.current_file.clone();
        let up_next = self.playlist.current_item().map(|i| i.path.clone());
        let is_video = self.player.media_info.as_ref().is_some_and(|m| m.primary_video_stream().is_some());
        let mut cx = BrowseCx {
            library: &self.library,
            art: &mut self.artwork,
            session: &self.session,
            now_playing: current.as_deref(),
            is_playing: self.player.is_playing,
            up_next: up_next.as_deref(),
            playlist_len: self.playlist.items.len(),
            playlist_open: self.show_playlist,
            live_frame: self.canvas.texture.as_ref().filter(|_| is_video),
            logo,
            levels,
            actions: &mut self.actions,
        };
        let menu = browse::show(ui, page_rect, &mut self.browse, &mut cx);

        // Details overlay, shown above everything on this screen.
        let mut details_closed = false;
        if let Some(path) = &self.details {
            match self.library.position(path) {
                Some(idx) => details_closed = browse::details_modal(ctx, &self.library.entries[idx], &mut cx),
                None => details_closed = true,
            }
        }
        if details_closed {
            self.details = None;
        }

        if let (Some(bar), Some(path), Some((title, subtitle))) = (bar_rect, current.as_deref(), &labels) {
            let out = mini_player::show(
                ui,
                bar,
                &mut self.player,
                MiniPlayerCx {
                    path,
                    title,
                    subtitle,
                    live_frame: self.canvas.texture.as_ref().filter(|_| is_video),
                    art: &mut self.artwork,
                },
            );
            if out.expand {
                self.view = View::Player;
            }
            if out.stop {
                self.close_media();
            }
            if out.next {
                self.play_next();
            }
            if out.prev {
                self.play_prev();
            }
        }

        self.app_menu(ctx, &menu);
    }

    /// Edge-to-edge playback with controls that fade away.
    fn player_screen(&mut self, ui: &mut Ui, ctx: &egui::Context, rect: Rect, subtitle: Option<&str>, levels: [f32; 16]) {
        let Some((title, sub_label)) = self.now_labels() else {
            self.view = View::Browse;
            return;
        };
        let audio_only = self.player.media_info.as_ref().is_some_and(|m| m.primary_video_stream().is_none());

        let idle = self.last_mouse_activity.elapsed().as_secs_f32() >= self.preferences_state.auto_hide_seconds;
        let keep_up = !self.player.is_playing
            || audio_only
            || self.controls_hovered
            || self.controls.is_seeking
            || self.show_playlist
            || self.any_dialog_open()
            || Popup::is_any_open(ctx);
        let controls_up = keep_up || !idle;
        let opacity = ctx.animate_bool_with_time(egui::Id::unique("player_controls_fade"), controls_up, 0.22);
        if ctx.input(|i| i.pointer.any_pressed()) {
            self.controls_up_at_press = opacity > 0.5;
        }

        let canvas = if audio_only {
            let path = self.player.current_file.clone().unwrap_or_default();
            self.canvas.show_audio(
                ui,
                rect,
                &mut self.artwork,
                AudioStage {
                    path: &path,
                    title: &title,
                    subtitle: &sub_label,
                    levels: &levels,
                    bottom_inset: BOTTOM_CONTROLS_H,
                },
            )
        } else {
            let lift = if controls_up { BOTTOM_CONTROLS_H + 12.0 } else { 0.0 };
            self.canvas.show_video(ui, rect, subtitle, self.preferences_state.subtitle_size, lift)
        };

        let touch = ctx.input(|i| i.has_touch_screen());
        if canvas.double_clicked() {
            self.toggle_fullscreen(ctx);
            if !touch {
                // The first click of the pair toggled playback; put it back.
                self.player.toggle_play();
            }
        } else if canvas.clicked() {
            if touch {
                // A tap shows or hides the controls; pausing is what the button is for.
                if self.controls_up_at_press {
                    self.last_mouse_activity = Instant::now() - Duration::from_secs(3600);
                }
            } else {
                self.player.toggle_play();
            }
        }
        if canvas.hovered() {
            self.adjust_volume_by_wheel(ctx);
            if opacity <= 0.0 {
                ctx.set_cursor_icon(egui::CursorIcon::None);
            }
        }

        self.controls.paint_flash(ui, rect, &self.player);

        let up_next = self.playlist.peek_next().map(|i| display_title(&i.path));
        let has_queue = self.playlist.items.len() > 1;
        let mut toast = None;
        let (mut toggle_fs, mut cycle_ab, mut open_jump, mut go_back, mut play_next, mut play_prev) =
            (false, false, false, false, false, false);
        let output = self.controls.show(
            ui,
            rect,
            opacity,
            &mut self.player,
            ControlsCtx {
                title: &title,
                subtitle: &sub_label,
                up_next: up_next.as_deref(),
                aspect_mode: &mut self.canvas.aspect_mode,
                show_playlist: &mut self.show_playlist,
                show_info: &mut self.show_info,
                show_preferences: &mut self.show_preferences,
                show_equalizer: &mut self.show_equalizer,
                is_fullscreen: self.is_fullscreen,
                toggle_fullscreen: &mut toggle_fs,
                toast_msg: &mut toast,
                prefs: &mut self.preferences_state,
                ab_loop: self.ab_loop,
                cycle_ab_loop: &mut cycle_ab,
                open_jump: &mut open_jump,
                go_back: &mut go_back,
                play_next: &mut play_next,
                play_prev: &mut play_prev,
                shuffle: &mut self.playlist.is_shuffle,
                repeat: &mut self.playlist.repeat_mode,
                has_queue,
                audio_only,
            },
        );

        self.controls_hovered = output.as_ref().is_some_and(|o| o.hovered);
        if self.controls_hovered {
            self.last_mouse_activity = Instant::now();
        }
        if let Some(t) = toast {
            self.set_toast(t);
        }
        if toggle_fs {
            self.toggle_fullscreen(ctx);
        }
        if cycle_ab {
            self.cycle_ab_loop();
        }
        if open_jump {
            self.open_jump_dialog();
        }
        if play_next {
            self.play_next();
        }
        if play_prev {
            self.play_prev();
        }
        if go_back {
            self.view = View::Browse;
        }
        if let Some(out) = output {
            self.app_menu(ctx, &out.menu);
        }
    }

    fn toggle_fullscreen(&mut self, ctx: &egui::Context) {
        self.is_fullscreen = !self.is_fullscreen;
        ctx.send_viewport_cmd(egui::ViewportCommand::Fullscreen(self.is_fullscreen));
        if self.is_fullscreen {
            self.set_toast("Press Esc to exit fullscreen");
        }
    }
}

impl eframe::App for AirMediaApp {
    fn ui(&mut self, ui: &mut Ui, _frame: &mut eframe::Frame) {
        let ctx = ui.ctx().clone();
        let logo_tex = self
            .logo_texture
            .get_or_insert_with(|| {
                let icon_bytes = include_bytes!("../assets/icon.png");
                let img = image::load_from_memory(icon_bytes)
                    .expect("Embedded icon.png must be valid")
                    .to_rgba8();
                let (w, h) = img.dimensions();
                let color_image = egui::ColorImage::from_rgba_unmultiplied(
                    [w as usize, h as usize],
                    &img.into_raw(),
                );
                let options = egui::TextureOptions { mipmap_mode: Some(egui::TextureFilter::Linear), ..egui::TextureOptions::LINEAR };
                ctx.load_texture("app_logo", color_image, options)
            })
            .clone();

        self.artwork.begin_frame(&ctx);
        self.handle_shortcuts(&ctx);
        self.handle_drag_and_drop(&ctx);
        self.player.poll_background();
        if let Some(err) = self.player.take_error() {
            self.set_error_toast(err);
        }
        self.track_current_file();
        if self.preferences_state.hw_accel != self.player.hw_accel {
            // Keep the decoder choice made via H / the settings chip for the next launch.
            self.preferences_state.hw_accel = self.player.hw_accel;
            self.preferences_state.save();
        }

        if let Some(frame) = self.player.take_new_video_frame() {
            self.canvas.update_texture(&ctx, &frame);
            ctx.request_repaint();
        }

        let (current_pointer, pressed) = ctx.input(|i| (i.pointer.hover_pos(), i.pointer.any_pressed()));
        if pressed {
            self.last_mouse_activity = Instant::now();
        }
        if let Some(pos) = current_pointer {
            if let Some(last_pos) = self.last_pointer_pos {
                if pos.distance(last_pos) > 2.0 {
                    self.last_mouse_activity = Instant::now();
                }
            } else {
                self.last_mouse_activity = Instant::now();
            }
            self.last_pointer_pos = Some(pos);
        }

        let active_subtitle = self.player.get_active_subtitle_text();
        let visualizer_levels = self.player.get_visualizer();

        self.handle_end_of_media();
        if self.player.is_playing && !self.controls.is_seeking {
            let t = self.player.current_time_secs();
            if let Some(a) = self.ab_loop.jump_target(t) {
                self.player.seek(a);
            }
        }

        let on_top = self.preferences_state.always_on_top;
        if self.applied_always_on_top != Some(on_top) {
            let level = if on_top { egui::WindowLevel::AlwaysOnTop } else { egui::WindowLevel::Normal };
            ctx.send_viewport_cmd(egui::ViewportCommand::WindowLevel(level));
            if self.applied_always_on_top.is_some() {
                self.preferences_state.save();
            }
            self.applied_always_on_top = Some(on_top);
        }

        let window_title = match self.now_labels() {
            Some((title, _)) => format!("{title} \u{2014} AirMedia"),
            None => "AirMedia".to_string(),
        };
        if window_title != self.window_title {
            ctx.send_viewport_cmd(egui::ViewportCommand::Title(window_title.clone()));
            self.window_title = window_title;
        }

        egui::CentralPanel::default()
            .frame(egui::Frame::NONE.fill(Theme::BG))
            .show(ui, |ui| {
                // The playlist drawer docks to the right of either screen.
                PlaylistView::show(ui, &mut self.playlist, &mut self.player, &mut self.show_playlist);

                let full_rect = ui.available_rect_before_wrap();
                let _ = ui.allocate_rect(full_rect, Sense::hover());
                let mut screen = ui.new_child(UiBuilder::new().id_salt("screen").max_rect(full_rect));
                screen.shrink_clip_rect(full_rect);
                match self.view {
                    View::Browse => self.browse_screen(&mut screen, &ctx, full_rect, &logo_tex, visualizer_levels),
                    View::Player => self.player_screen(&mut screen, &ctx, full_rect, active_subtitle.as_deref(), visualizer_levels),
                }

                // Toast HUD
                if let Some((msg, time, duration)) = &self.toast {
                    let elapsed = time.elapsed().as_secs_f32();
                    if elapsed < *duration {
                        let fade_start = duration - 0.4;
                        let alpha = (1.0 - (elapsed - fade_start).max(0.0) / 0.4).clamp(0.0, 1.0);
                        let pos = Pos2::new(full_rect.center().x, full_rect.min.y + 84.0);
                        draw_toast_hud(ui, pos, msg, alpha);
                    } else {
                        self.toast = None;
                    }
                }

                if ctx.input(|i| !i.raw.hovered_files.is_empty()) {
                    draw_drag_overlay(ui, full_rect);
                }
            });

        // Dialogs
        InfoDialog::show(ui, &self.player, &mut self.show_info);
        let effects = EqualizerDialog::show(ui, &mut self.session.equalizer, &mut self.show_equalizer);
        if effects.audio {
            self.player.set_audio_filter(self.session.equalizer.audio_filter());
        }
        if effects.video {
            self.player.set_video_filter(self.session.equalizer.video_filter());
        }
        if let Some(url) = StreamDialog::show(ui, &mut self.stream_url_input, &mut self.show_stream) {
            self.open_paths(vec![url], true);
        }
        PreferencesDialog::show(ui, &mut self.player, &mut self.preferences_state, &mut self.show_preferences);
        AboutDialog::show(ui, &mut self.show_about, &logo_tex);
        ShortcutsDialog::show(ui, &mut self.show_shortcuts);
        let duration = self.player.duration_secs;
        if let Some(t) = JumpDialog::show(ui, &mut self.jump_input, &mut self.show_jump, duration) {
            self.player.seek(t);
            self.set_toast(format!("Jumped to {}", format_clock(t)));
        }
        CodecDialog::show(ui, &mut self.codec_state, &mut self.show_codecs);

        if !self.actions.is_empty() {
            self.run_actions(&ctx);
        }

        if self.last_session_save.elapsed() >= SESSION_SAVE_INTERVAL
            || ctx.input(|i| i.viewport().close_requested())
        {
            self.save_session();
        }

        let video_on_screen = self.view == View::Player
            && self.player.media_info.as_ref().is_some_and(|m| m.primary_video_stream().is_some());
        if self.player.is_playing && video_on_screen {
            ctx.request_repaint();
        } else if self.player.is_playing {
            // Music and the mini player don't need every display refresh.
            ctx.request_repaint_after(Duration::from_millis(33));
        } else if self.toast.is_some() || self.codec_state.downloader.is_busy() {
            ctx.request_repaint();
        } else {
            // Keep the auto-hide timer and fades ticking without spinning the CPU.
            ctx.request_repaint_after(Duration::from_millis(250));
        }
    }
}

impl Drop for AirMediaApp {
    fn drop(&mut self) {
        self.save_session();
    }
}

fn format_clock(secs: f64) -> String {
    let total = secs.max(0.0) as u64;
    let (h, m, s) = (total / 3600, (total / 60) % 60, total % 60);
    if h > 0 {
        format!("{h}:{m:02}:{s:02}")
    } else {
        format!("{m}:{s:02}")
    }
}


fn draw_toast_hud(ui: &mut Ui, center: Pos2, text: &str, alpha: f32) {
    let mut painter = ui
        .painter()
        .clone()
        .with_layer_id(egui::LayerId::new(egui::Order::Foreground, egui::Id::unique("toast")));
    painter.multiply_opacity(alpha);
    let galley = painter.layout_no_wrap(text.to_owned(), Theme::font_semibold(13.5), Theme::TEXT);
    let rect = Rect::from_center_size(center, galley.size() + Vec2::new(32.0, 16.0));
    painter.rect_filled(rect, Theme::radius_full(), Theme::black(190));
    painter.galley(rect.center() - galley.size() / 2.0, galley, Theme::TEXT);
}

fn draw_drag_overlay(ui: &mut Ui, rect: Rect) {
    let painter = ui.painter();
    painter.rect_filled(rect, egui::CornerRadius::ZERO, Theme::black(200));
    painter.rect_stroke(
        rect.shrink(20.0),
        Theme::radius_lg(),
        Stroke::new(1.5, Theme::ACCENT_SOFT),
        Theme::stroke_kind(),
    );
    let center = rect.center();
    painter.text(
        center - Vec2::new(0.0, 12.0),
        Align2::CENTER_CENTER,
        "Drop to play",
        Theme::font_semibold(22.0),
        Theme::TEXT,
    );
    painter.text(
        center + Vec2::new(0.0, 18.0),
        Align2::CENTER_CENTER,
        "Video, audio or subtitle files",
        Theme::font(13.5),
        Theme::TEXT_MUTED,
    );
}
