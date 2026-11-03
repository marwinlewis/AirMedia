use crate::media::decoder::HwAccelMode;
use crate::media::MediaPlayer;
use crate::playlist::Playlist;
use crate::ui::about_dialog::AboutDialog;
use crate::ui::codec_dialog::{CodecDialog, CodecDialogState};
use crate::ui::canvas::{AspectRatioMode, VideoCanvas};
use crate::ui::controls::{ControlsCtx, ControlsView};
use crate::ui::equalizer_dialog::{EqualizerDialog, EqualizerState};
use crate::ui::info_dialog::InfoDialog;
use crate::ui::icons::Icon;
use crate::ui::menu_bar::{MenuBarView, MenuCtx, MEDIA_EXTENSIONS};
use crate::ui::playlist_view::PlaylistView;
use crate::ui::preferences_dialog::{PreferencesDialog, PreferencesState};
use crate::ui::stream_dialog::StreamDialog;
use crate::ui::theme::Theme;
use crate::ui::top_bar::TopBarView;

use eframe::egui::{self, Align2, Key, Popup, PopupCloseBehavior, Pos2, Rect, RichText, Stroke, Ui, Vec2};
use std::path::PathBuf;
use std::time::{Duration, Instant};

pub struct AirMediaApp {
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
    pub codec_state: CodecDialogState,
    pub equalizer_state: EqualizerState,
    pub preferences_state: PreferencesState,
    pub recent_files: Vec<PathBuf>,
    pub stream_url_input: String,
    pub is_fullscreen: bool,
    pub last_mouse_activity: Instant,
    pub last_pointer_pos: Option<Pos2>,
    pub toast: Option<(String, Instant)>,
    window_title: String,
}

enum IdleAction {
    OpenFile,
    OpenStream,
}

impl AirMediaApp {
    pub fn new() -> Self {
        let codec_state = CodecDialogState::default();
        let show_codecs = !codec_state.status.is_ready();
        let preferences_state = PreferencesState::load();
        Self {
            player: MediaPlayer::new(),
            playlist: Playlist::new(),
            canvas: VideoCanvas::new(),
            controls: ControlsView::new(),
            show_playlist: false,
            show_info: false,
            show_equalizer: false,
            show_stream: false,
            show_preferences: false,
            show_about: false,
            show_codecs,
            codec_state,
            equalizer_state: EqualizerState::default(),
            preferences_state,
            recent_files: Vec::new(),
            stream_url_input: String::new(),
            is_fullscreen: false,
            last_mouse_activity: Instant::now(),
            last_pointer_pos: None,
            toast: None,
            window_title: String::new(),
        }
    }

    pub fn set_toast(&mut self, msg: impl Into<String>) {
        self.toast = Some((msg.into(), Instant::now()));
    }

    fn handle_shortcuts(&mut self, ctx: &egui::Context) {
        if ctx.egui_wants_keyboard_input() {
            return;
        }
        let input = ctx.input(|i| i.clone());

        if input.modifiers.ctrl && input.key_pressed(Key::Q) {
            ctx.send_viewport_cmd(egui::ViewportCommand::Close);
        }

        // Fullscreen toggle: F or F11
        if input.key_pressed(Key::F) || input.key_pressed(Key::F11) {
            self.toggle_fullscreen(ctx);
        }

        // Exit fullscreen on Esc
        if input.key_pressed(Key::Escape) && self.is_fullscreen {
            self.is_fullscreen = false;
            ctx.send_viewport_cmd(egui::ViewportCommand::Fullscreen(false));
        }

        // Space: Play / Pause
        if input.key_pressed(Key::Space) {
            self.player.toggle_play();
            if self.player.is_playing {
                self.set_toast("Playing");
            } else {
                self.set_toast("Paused");
            }
        }

        // S: Stop
        if input.key_pressed(Key::S) && !input.modifiers.ctrl && !input.modifiers.shift {
            self.player.stop();
            self.set_toast("Stopped");
        }

        // Shift+S: Take Screenshot
        if input.modifiers.shift && input.key_pressed(Key::S) {
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

        if input.key_pressed(Key::ArrowLeft) {
            let cur = self.player.current_time_secs();
            self.player.seek(cur - seek_delta);
            self.set_toast(format!("− {seek_delta} s"));
        }
        if input.key_pressed(Key::ArrowRight) {
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
        if input.key_pressed(Key::Period) {
            self.player.step_frame(true);
            self.set_toast("Next frame");
        }
        if input.key_pressed(Key::Comma) {
            self.player.step_frame(false);
            self.set_toast("Previous frame");
        }

        // Speed: [ and ]
        if input.key_pressed(Key::OpenBracket) {
            let spd = (self.player.playback_speed - 0.25).max(0.25);
            self.player.set_speed(spd);
            self.set_toast(format!("Speed {spd}×"));
        }
        if input.key_pressed(Key::CloseBracket) {
            let spd = (self.player.playback_speed + 0.25).min(2.5);
            self.player.set_speed(spd);
            self.set_toast(format!("Speed {spd}×"));
        }
        if input.key_pressed(Key::Backspace) || input.key_pressed(Key::Equals) {
            self.player.set_speed(1.0);
            self.set_toast("Speed 1×");
        }

        // Playlist Next / Prev: N / P
        if input.key_pressed(Key::N) && !ctrl {
            if let Some(item) = self.playlist.next() {
                let path = item.path.clone();
                let title = item.title.clone();
                let _ = self.player.load_file(&path);
                self.set_toast(format!("Next: {title}"));
            }
        }
        if input.key_pressed(Key::P) && !ctrl {
            if let Some(item) = self.playlist.prev() {
                let path = item.path.clone();
                let title = item.title.clone();
                let _ = self.player.load_file(&path);
                self.set_toast(format!("Prev: {title}"));
            }
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

        // Ctrl+O: Open file
        if input.key_pressed(Key::O) && !ctrl {
            self.open_file_dialog();
        } else if input.modifiers.ctrl && input.key_pressed(Key::O) {
            self.open_file_dialog();
        }

        // A: Cycle Aspect Ratio
        if input.key_pressed(Key::A) {
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
        if input.key_pressed(Key::H) {
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
        if let Some(path) = crate::ui::FileDialog::new()
            .add_filter("Media", &MEDIA_EXTENSIONS)
            .pick_file()
        {
            self.playlist.add(&path);
            self.playlist.current_index = Some(self.playlist.items.len() - 1);
            let _ = self.player.load_file(&path);
            if !self.recent_files.contains(&path) {
                self.recent_files.push(path);
            }
        }
    }

    fn handle_drag_and_drop(&mut self, ctx: &egui::Context) {
        let dropped_files = ctx.input(|i| i.raw.dropped_files.clone());
        if !dropped_files.is_empty() {
            for f in dropped_files {
                let path = f.path().to_path_buf();
                let ext = path
                    .extension()
                    .and_then(|e| e.to_str())
                    .unwrap_or("")
                    .to_lowercase();

                if Playlist::is_supported_subtitle_extension(&ext) {
                    let _ = self.player.load_external_subtitle(&path);
                    self.set_toast("Subtitles loaded");
                } else {
                    self.playlist.add(&path);
                    self.playlist.current_index = Some(self.playlist.items.len() - 1);
                    let _ = self.player.load_file(&path);
                    if let Some(name) = path.file_name() {
                        self.set_toast(format!("Loaded: {}", name.to_string_lossy()));
                    }
                    if !self.recent_files.contains(&path) {
                        self.recent_files.push(path);
                    }
                }
            }
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

        self.handle_shortcuts(&ctx);
        self.handle_drag_and_drop(&ctx);

        if let Some(frame) = self.player.take_new_video_frame() {
            self.canvas.update_texture(&ctx, &frame);
            ctx.request_repaint();
        }

        let current_pointer = ctx.input(|i| i.pointer.hover_pos());
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

        let is_audio_only = self
            .player
            .media_info
            .as_ref()
            .is_some_and(|m| m.primary_video_stream().is_none());
        let has_media = self.player.current_file.is_some();

        let active_subtitle = self.player.get_active_subtitle_text();
        let visualizer_levels = self.player.get_visualizer();

        if self.player.is_playing && self.player.duration_secs > 0.5 {
            let cur = self.player.current_time_secs();
            if cur >= self.player.duration_secs - 0.25 {
                if let Some(next_item) = self.playlist.next() {
                    let path = next_item.path.clone();
                    let title = next_item.title.clone();
                    let _ = self.player.load_file(&path);
                    self.set_toast(format!("Playing: {title}"));
                } else {
                    self.player.stop();
                }
            }
        }

        let title = self
            .playlist
            .current_item()
            .map(|i| i.title.clone())
            .unwrap_or_default();
        if title != self.window_title {
            let window_title = if title.is_empty() {
                "AirMedia".to_string()
            } else {
                format!("{title} — AirMedia")
            };
            ctx.send_viewport_cmd(egui::ViewportCommand::Title(window_title));
            self.window_title = title.clone();
        }

        let mut idle_action = None;

        egui::CentralPanel::default()
            .frame(egui::Frame::NONE.fill(Theme::BG))
            .show(ui, |ui| {
                // Playlist drawer docks to the right; the video takes the rest.
                PlaylistView::show(ui, &mut self.playlist, &mut self.player, &mut self.show_playlist);

                let full_rect = ui.available_rect_before_wrap();

                let canvas_resp = self.canvas.show(
                    ui,
                    active_subtitle.as_deref(),
                    self.preferences_state.subtitle_size,
                    is_audio_only,
                    &visualizer_levels,
                );

                if has_media && canvas_resp.double_clicked() {
                    self.toggle_fullscreen(&ctx);
                } else if has_media && canvas_resp.clicked() {
                    self.player.toggle_play();
                }

                // Wheel over the video adjusts volume.
                if canvas_resp.hovered() && !Popup::is_any_open(&ctx) {
                    let scroll = ctx.input(|i| i.smooth_scroll_delta.y);
                    if scroll.abs() > 2.0 {
                        let delta = if scroll > 0.0 { 0.05 } else { -0.05 };
                        let new_vol = (self.player.get_volume() + delta).clamp(0.0, 1.5);
                        self.player.set_volume(new_vol);
                        self.set_toast(format!("Volume {}%", (new_vol * 100.0).round() as u32));
                    }
                }

                if !has_media {
                    idle_action = draw_empty_state(ui, full_rect);
                } else if is_audio_only {
                    ui.painter().text(
                        full_rect.center() + Vec2::new(0.0, 120.0),
                        Align2::CENTER_CENTER,
                        &title,
                        Theme::font_semibold(16.0),
                        Theme::TEXT_MUTED,
                    );
                }

                // Overlay chrome (top bar + bottom controls), fading with mouse activity.
                let timeout = Duration::from_secs_f32(self.preferences_state.auto_hide_seconds);
                let recently_active = self.last_mouse_activity.elapsed() < timeout;
                let pointer_in_window = current_pointer.is_some_and(|p| full_rect.contains(p));
                let keep_visible = !self.player.is_playing
                    || self.controls.is_seeking
                    || Popup::is_any_open(&ctx)
                    || (recently_active && pointer_in_window);
                let chrome_alpha =
                    ctx.animate_bool_with_time(egui::Id::unique("chrome_alpha"), keep_visible, 0.25);

                if chrome_alpha <= 0.0 && pointer_in_window {
                    ctx.set_cursor_icon(egui::CursorIcon::None);
                }

                if chrome_alpha > 0.0 {
                    let mut overlay = ui.new_child(egui::UiBuilder::new().max_rect(full_rect));
                    overlay.multiply_opacity(chrome_alpha);

                    let menu_resp = TopBarView::show(&mut overlay, full_rect, &self.player, &title);
                    let mut menu_toast = None;
                    let mut toggle_fs = false;
                    Popup::menu(&menu_resp)
                        .close_behavior(PopupCloseBehavior::CloseOnClickOutside)
                        .gap(6.0)
                        .show(|ui| {
                            MenuBarView::show(
                                ui,
                                &mut self.player,
                                &mut self.playlist,
                                MenuCtx {
                                    recent_files: &mut self.recent_files,
                                    aspect_mode: &mut self.canvas.aspect_mode,
                                    show_playlist: &mut self.show_playlist,
                                    show_info: &mut self.show_info,
                                    show_equalizer: &mut self.show_equalizer,
                                    show_stream: &mut self.show_stream,
                                    show_preferences: &mut self.show_preferences,
                                    show_about: &mut self.show_about,
                                    show_codecs: &mut self.show_codecs,
                                    toggle_fullscreen: &mut toggle_fs,
                                    toast_msg: &mut menu_toast,
                                    prefs: &self.preferences_state,
                                },
                            );
                        });

                    if has_media {
                        let mut controls_toast = None;
                        let hovering_controls = self.controls.show(
                            &mut overlay,
                            full_rect,
                            &mut self.player,
                            &mut self.playlist,
                            ControlsCtx {
                                aspect_mode: &mut self.canvas.aspect_mode,
                                show_playlist: &mut self.show_playlist,
                                show_info: &mut self.show_info,
                                show_preferences: &mut self.show_preferences,
                                is_fullscreen: self.is_fullscreen,
                                toggle_fullscreen: &mut toggle_fs,
                                toast_msg: &mut controls_toast,
                                prefs: &mut self.preferences_state,
                            },
                        );
                        if let Some(t) = controls_toast {
                            self.set_toast(t);
                        }
                        if hovering_controls {
                            self.last_mouse_activity = Instant::now();
                        }
                    }

                    if let Some(t) = menu_toast {
                        self.set_toast(t);
                    }
                    if toggle_fs {
                        self.toggle_fullscreen(&ctx);
                    }
                }

                // Toast HUD
                if let Some((msg, time)) = &self.toast {
                    let elapsed = time.elapsed().as_secs_f32();
                    if elapsed < 1.4 {
                        let alpha = (1.0 - (elapsed - 1.0).max(0.0) / 0.4).clamp(0.0, 1.0);
                        let pos = Pos2::new(full_rect.center().x, full_rect.min.y + 72.0);
                        draw_toast_hud(ui, pos, msg, alpha);
                    } else {
                        self.toast = None;
                    }
                }

                if ctx.input(|i| !i.raw.hovered_files.is_empty()) {
                    draw_drag_overlay(ui, full_rect);
                }
            });

        match idle_action {
            Some(IdleAction::OpenFile) => self.open_file_dialog(),
            Some(IdleAction::OpenStream) => self.show_stream = true,
            None => {}
        }

        // Dialogs
        InfoDialog::show(ui, &self.player, &mut self.show_info);
        EqualizerDialog::show(ui, &mut self.equalizer_state, &mut self.show_equalizer);
        StreamDialog::show(
            ui,
            &mut self.player,
            &mut self.playlist,
            &mut self.stream_url_input,
            &mut self.show_stream,
        );
        PreferencesDialog::show(ui, &mut self.player, &mut self.preferences_state, &mut self.show_preferences);
        AboutDialog::show(ui, &mut self.show_about);
        CodecDialog::show(ui, &mut self.codec_state, &mut self.show_codecs);

        if self.player.is_playing || self.toast.is_some() || self.codec_state.downloader.is_busy() {
            ctx.request_repaint();
        } else {
            // Keep the auto-hide timer and fades ticking without spinning the CPU.
            ctx.request_repaint_after(Duration::from_millis(250));
        }
    }
}

fn draw_empty_state(ui: &mut Ui, area: Rect) -> Option<IdleAction> {
    let mut action = None;
    let content = Rect::from_center_size(area.center() - Vec2::new(0.0, 12.0), Vec2::new(380.0, 300.0));
    let mut col = ui.new_child(
        egui::UiBuilder::new()
            .max_rect(content)
            .layout(egui::Layout::top_down(egui::Align::Center)),
    );

    // Logo mark
    let (logo, _) = col.allocate_exact_size(Vec2::splat(64.0), egui::Sense::hover());
    col.painter().rect_filled(logo, egui::CornerRadius::same(18), Theme::ACCENT);
    Icon::Play.paint(col.painter(), logo.shrink(18.0).translate(Vec2::new(2.0, 0.0)), Theme::TEXT);

    col.add_space(18.0);
    col.label(RichText::new("AirMedia").font(Theme::font_semibold(26.0)).color(Theme::TEXT));
    col.add_space(2.0);
    col.label(RichText::new("Drop a file anywhere to start playing").size(14.0).color(Theme::TEXT_MUTED));
    col.add_space(22.0);

    // Center the two buttons as a group.
    let btn_w = 132.0;
    let gap = 10.0;
    let (row_rect, _) = col.allocate_exact_size(Vec2::new(btn_w * 2.0 + gap, 38.0), egui::Sense::hover());
    let mut row = col.new_child(
        egui::UiBuilder::new()
            .max_rect(row_rect)
            .layout(egui::Layout::left_to_right(egui::Align::Center)),
    );
    row.spacing_mut().item_spacing.x = gap;
    let primary = egui::Button::new(RichText::new("Open file").font(Theme::font_semibold(14.0)).color(Theme::TEXT))
        .fill(Theme::ACCENT)
        .corner_radius(Theme::radius_md())
        .min_size(Vec2::new(btn_w, 38.0));
    if row.add(primary).clicked() {
        action = Some(IdleAction::OpenFile);
    }
    let secondary = egui::Button::new(RichText::new("Open stream").size(14.0).color(Theme::TEXT))
        .fill(Theme::white(14))
        .corner_radius(Theme::radius_md())
        .min_size(Vec2::new(btn_w, 38.0));
    if row.add(secondary).clicked() {
        action = Some(IdleAction::OpenStream);
    }

    col.add_space(26.0);
    let mut job = egui::text::LayoutJob::default();
    for (i, (key, what)) in [("O", "open"), ("L", "playlist"), ("F", "fullscreen")].iter().enumerate() {
        if i > 0 {
            job.append("      ", 0.0, egui::TextFormat::default());
        }
        job.append(
            key,
            0.0,
            egui::TextFormat {
                font_id: Theme::font_semibold(12.0),
                color: Theme::TEXT_MUTED,
                ..Default::default()
            },
        );
        job.append(
            &format!("  {what}"),
            0.0,
            egui::TextFormat {
                font_id: Theme::font(12.0),
                color: Theme::TEXT_FAINT,
                ..Default::default()
            },
        );
    }
    col.label(job);

    action
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
