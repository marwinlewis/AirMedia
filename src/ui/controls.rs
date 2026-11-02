use super::canvas::AspectRatioMode;
use super::icons::{icon_button, icon_button_ex, Icon};
use super::preferences_dialog::PreferencesState;
use super::theme::Theme;
use crate::media::decoder::HwAccelMode;
use crate::media::{MediaPlayer, ScreenshotFormat};
use crate::playlist::Playlist;
use eframe::egui::{
    self, Align2, Popup, PopupCloseBehavior, Pos2, Rect, RectAlign, RichText, Sense, Ui, Vec2,
};

pub const SPEEDS: [f32; 8] = [0.25, 0.5, 0.75, 1.0, 1.25, 1.5, 2.0, 2.5];

/// Height of the bottom overlay (scrim + seek bar + button row).
pub const CONTROLS_HEIGHT: f32 = 132.0;

pub struct ControlsView {
    pub show_remaining_time: bool,
    pub is_seeking: bool,
    pub seek_preview_secs: Option<f64>,
}

/// Everything the bottom bar can change outside of the player itself.
pub struct ControlsCtx<'a> {
    pub aspect_mode: &'a mut AspectRatioMode,
    pub show_playlist: &'a mut bool,
    pub show_info: &'a mut bool,
    pub show_preferences: &'a mut bool,
    pub is_fullscreen: bool,
    pub toggle_fullscreen: &'a mut bool,
    pub toast_msg: &'a mut Option<String>,
    pub prefs: &'a mut PreferencesState,
}

impl ControlsView {
    pub fn new() -> Self {
        Self {
            show_remaining_time: false,
            is_seeking: false,
            seek_preview_secs: None,
        }
    }

    /// Paints the overlay inside `area` (the full canvas rect). Returns true if the pointer is
    /// over the interactive part, so the caller can keep the controls visible.
    pub fn show(
        &mut self,
        ui: &mut Ui,
        area: Rect,
        player: &mut MediaPlayer,
        playlist: &mut Playlist,
        cx: ControlsCtx<'_>,
    ) -> bool {
        let scrim = Rect::from_min_max(Pos2::new(area.min.x, area.max.y - CONTROLS_HEIGHT), area.max);
        Theme::paint_vertical_gradient(ui.painter(), scrim, Theme::black(0), Theme::black(190));

        let inner = scrim.shrink2(Vec2::new(20.0, 0.0));
        let seek_rect = Rect::from_min_max(
            Pos2::new(inner.min.x, inner.max.y - 74.0),
            Pos2::new(inner.max.x, inner.max.y - 54.0),
        );
        let row_rect = Rect::from_min_max(
            Pos2::new(inner.min.x - 6.0, inner.max.y - 50.0),
            Pos2::new(inner.max.x + 6.0, inner.max.y - 10.0),
        );

        let cur_time = player.current_time_secs();
        let dur = player.duration_secs;

        let mut seek_ui = ui.new_child(egui::UiBuilder::new().max_rect(seek_rect));
        self.seek_bar(&mut seek_ui, cur_time, dur, player);

        let mut row = ui.new_child(
            egui::UiBuilder::new()
                .max_rect(row_rect)
                .layout(egui::Layout::left_to_right(egui::Align::Center)),
        );
        row.spacing_mut().item_spacing.x = 2.0;

        // Left cluster: transport, volume, time
        if icon_button(&mut row, Icon::Prev, 34.0).on_hover_text("Previous track (P)").clicked() {
            if let Some(item) = playlist.prev() {
                let _ = player.load_file(&item.path);
            }
        }
        if icon_button(&mut row, Icon::StepBackward, 34.0).on_hover_text("Previous frame (,)").clicked() {
            player.step_frame(false);
            *cx.toast_msg = Some("Previous frame".to_string());
        }
        let play_icon = if player.is_playing { Icon::Pause } else { Icon::Play };
        if icon_button(&mut row, play_icon, 40.0)
            .on_hover_text(if player.is_playing { "Pause (Space)" } else { "Play (Space)" })
            .clicked()
        {
            player.toggle_play();
        }
        if icon_button(&mut row, Icon::StepForward, 34.0).on_hover_text("Advance frame (.)").clicked() {
            player.step_frame(true);
            *cx.toast_msg = Some("Next frame".to_string());
        }
        if icon_button(&mut row, Icon::Next, 34.0).on_hover_text("Next track (N)").clicked() {
            if let Some(item) = playlist.next() {
                let _ = player.load_file(&item.path);
            }
        }

        row.add_space(4.0);
        let vol = player.get_volume();
        let muted = player.is_muted();
        let vol_icon = if muted || vol <= 0.001 {
            Icon::VolumeMute
        } else if vol < 0.5 {
            Icon::VolumeLow
        } else {
            Icon::VolumeHigh
        };
        if icon_button(&mut row, vol_icon, 36.0).on_hover_text("Mute (M)").clicked() {
            player.toggle_mute();
        }
        let mut shown_vol = if muted { 0.0 } else { vol };
        if volume_slider(&mut row, &mut shown_vol) {
            if muted {
                player.toggle_mute();
            }
            player.set_volume(shown_vol);
        }

        row.add_space(12.0);
        let shown_time = self.seek_preview_secs.unwrap_or(cur_time);
        let (cur_txt, total_txt) = if self.show_remaining_time && dur > 0.0 {
            (format_time(shown_time), format!("-{}", format_time((dur - shown_time).max(0.0))))
        } else {
            (format_time(shown_time), format_time(dur))
        };
        let mut job = egui::text::LayoutJob::default();
        let fmt = |color| egui::TextFormat {
            font_id: Theme::font(13.0),
            color,
            ..Default::default()
        };
        job.append(&cur_txt, 0.0, fmt(Theme::TEXT));
        job.append("  /  ", 0.0, fmt(Theme::TEXT_FAINT));
        job.append(&total_txt, 0.0, fmt(Theme::TEXT_MUTED));
        let time_resp = row
            .add(egui::Label::new(job).sense(Sense::click()).selectable(false))
            .on_hover_text("Toggle remaining time");
        if time_resp.clicked() {
            self.show_remaining_time = !self.show_remaining_time;
        }

        // Right cluster
        row.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            ui.spacing_mut().item_spacing.x = 2.0;

            let fs_icon = if cx.is_fullscreen { Icon::ExitFullscreen } else { Icon::Fullscreen };
            if icon_button(ui, fs_icon, 36.0).on_hover_text("Fullscreen (F)").clicked() {
                *cx.toggle_fullscreen = true;
            }

            if icon_button_ex(ui, Icon::Playlist, 36.0, *cx.show_playlist)
                .on_hover_text("Playlist (L)")
                .clicked()
            {
                *cx.show_playlist = !*cx.show_playlist;
            }

            let settings_resp = icon_button(ui, Icon::Settings, 36.0).on_hover_text("Settings");
            popup_above(&settings_resp).width(272.0).show(|ui| {
                settings_menu(ui, player, cx.aspect_mode, cx.show_info, cx.show_preferences, cx.prefs);
            });

            let subs_active = player.active_subtitle_track.is_some();
            let subs_resp = icon_button_ex(ui, Icon::Subtitles, 36.0, subs_active).on_hover_text("Subtitles");
            popup_above(&subs_resp).width(260.0).show(|ui| {
                subtitles_menu(ui, player);
            });

            if icon_button(ui, Icon::Camera, 36.0).on_hover_text("Take screenshot (Shift+S)").clicked() {
                match player.take_screenshot(&cx.prefs.screenshot_dir, cx.prefs.screenshot_format) {
                    Ok(path) => *cx.toast_msg = Some(format!("Screenshot saved: {}", path.display())),
                    Err(e) => *cx.toast_msg = Some(format!("Screenshot failed: {e}")),
                }
            }

            if (player.playback_speed - 1.0).abs() > 0.01 {
                ui.label(
                    RichText::new(format!("{}×", trim_speed(player.playback_speed)))
                        .color(Theme::ACCENT_SOFT)
                        .size(12.5),
                );
                ui.add_space(6.0);
            }
        });

        let hovered = ui
            .ctx()
            .pointer_hover_pos()
            .is_some_and(|p| Rect::from_min_max(seek_rect.min - Vec2::new(0.0, 10.0), row_rect.max).contains(p));
        hovered || self.is_seeking
    }

    fn seek_bar(&mut self, ui: &mut Ui, cur_time: f64, total: f64, player: &mut MediaPlayer) {
        let rect = ui.max_rect();
        let resp = ui.allocate_rect(rect, Sense::click_and_drag());
        let painter = ui.painter();

        let hot = resp.hovered() || resp.dragged();
        let t = ui.ctx().animate_bool_with_time(resp.id, hot, 0.12);
        let rail_h = 3.0 + 2.0 * t;
        let rail = Rect::from_center_size(rect.center(), Vec2::new(rect.width(), rail_h));

        let pointer_frac = resp
            .hover_pos()
            .or(resp.interact_pointer_pos())
            .map(|p| ((p.x - rail.min.x) / rail.width()).clamp(0.0, 1.0));

        if (resp.drag_started() || resp.dragged()) && total > 0.0 {
            self.is_seeking = true;
            if let Some(f) = pointer_frac {
                self.seek_preview_secs = Some(f as f64 * total);
            }
        }
        if (resp.drag_stopped() || resp.clicked()) && total > 0.0 {
            if let Some(f) = pointer_frac {
                player.seek(f as f64 * total);
            }
            self.is_seeking = false;
            self.seek_preview_secs = None;
        }

        let shown = self.seek_preview_secs.unwrap_or(cur_time);
        let progress = if total > 0.0 { (shown / total).clamp(0.0, 1.0) as f32 } else { 0.0 };
        let r = Theme::radius_full();

        painter.rect_filled(rail, r, Theme::white(46));
        if let (Some(f), true) = (pointer_frac, hot) {
            let hover_rect = Rect::from_min_max(rail.min, Pos2::new(rail.min.x + rail.width() * f, rail.max.y));
            painter.rect_filled(hover_rect, r, Theme::white(40));
        }
        let fill = Rect::from_min_max(rail.min, Pos2::new(rail.min.x + rail.width() * progress, rail.max.y));
        painter.rect_filled(fill, r, Theme::ACCENT);

        if t > 0.0 {
            let knob = Pos2::new(fill.max.x, rail.center().y);
            painter.circle_filled(knob, 6.5 * t, Theme::TEXT);
        }

        // Floating time bubble
        if let (Some(f), true, true) = (pointer_frac, hot, total > 0.0) {
            let secs = f as f64 * total;
            let text = format_time(secs);
            let galley = painter.layout_no_wrap(text, Theme::font(12.0), Theme::TEXT);
            let size = galley.size() + Vec2::new(16.0, 8.0);
            let x = (rail.min.x + rail.width() * f).clamp(rail.min.x + size.x / 2.0, rail.max.x - size.x / 2.0);
            let bubble = Rect::from_center_size(Pos2::new(x, rail.min.y - 20.0), size);
            painter.rect_filled(bubble, Theme::radius_sm(), Theme::black(220));
            painter.galley(bubble.center() - galley.size() / 2.0, galley, Theme::TEXT);
        }

        if hot {
            ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand);
        }
    }
}

/// Compact custom volume slider (0–150%). Returns true when the value changed.
fn volume_slider(ui: &mut Ui, value: &mut f32) -> bool {
    let (rect, resp) = ui.allocate_exact_size(Vec2::new(88.0, 28.0), Sense::click_and_drag());
    let max = 1.5;
    let mut changed = false;
    if let Some(p) = resp.interact_pointer_pos() {
        if resp.dragged() || resp.clicked() || resp.is_pointer_button_down_on() {
            let f = ((p.x - rect.min.x) / rect.width()).clamp(0.0, 1.0);
            let nv = (f * max * 100.0).round() / 100.0;
            if (nv - *value).abs() > f32::EPSILON {
                *value = nv;
                changed = true;
            }
        }
    }

    let painter = ui.painter();
    let t = ui.ctx().animate_bool_with_time(resp.id, resp.hovered() || resp.dragged(), 0.12);
    let rail = Rect::from_center_size(rect.center(), Vec2::new(rect.width(), 4.0));
    let r = Theme::radius_full();
    painter.rect_filled(rail, r, Theme::white(46));
    let frac = (*value / max).clamp(0.0, 1.0);
    let fill = Rect::from_min_max(rail.min, Pos2::new(rail.min.x + rail.width() * frac, rail.max.y));
    let fill_color = if *value > 1.0 { Theme::ACCENT_SOFT } else { Theme::TEXT };
    painter.rect_filled(fill, r, fill_color);
    // 100% marker
    let mark_x = rail.min.x + rail.width() / max;
    painter.line_segment(
        [Pos2::new(mark_x, rail.min.y - 3.0), Pos2::new(mark_x, rail.max.y + 3.0)],
        egui::Stroke::new(1.0, Theme::white(70)),
    );
    painter.circle_filled(Pos2::new(fill.max.x, rail.center().y), 4.0 + 2.0 * t, Theme::TEXT);

    resp.on_hover_text(format!("Volume {}%", (*value * 100.0).round() as u32));
    changed
}

fn popup_above(resp: &egui::Response) -> Popup<'_> {
    Popup::menu(resp)
        .close_behavior(PopupCloseBehavior::CloseOnClickOutside)
        .align(RectAlign::TOP_END)
        .gap(10.0)
}

pub fn section_label(ui: &mut Ui, text: &str) {
    ui.add_space(4.0);
    ui.label(RichText::new(text.to_uppercase()).size(10.5).color(Theme::TEXT_FAINT).strong());
    ui.add_space(2.0);
}

fn settings_menu(
    ui: &mut Ui,
    player: &mut MediaPlayer,
    aspect: &mut AspectRatioMode,
    show_info: &mut bool,
    show_preferences: &mut bool,
    prefs: &mut PreferencesState,
) {
    ui.spacing_mut().item_spacing = Vec2::new(4.0, 4.0);

    section_label(ui, "Playback speed");
    ui.horizontal_wrapped(|ui| {
        for s in SPEEDS {
            let selected = (player.playback_speed - s).abs() < 0.01;
            if chip(ui, selected, &format!("{}×", trim_speed(s))).clicked() {
                player.set_speed(s);
            }
        }
    });

    ui.add_space(6.0);
    section_label(ui, "Aspect ratio");
    ui.horizontal_wrapped(|ui| {
        for mode in AspectRatioMode::ALL {
            if chip(ui, *aspect == mode, mode.short_name()).clicked() {
                *aspect = mode;
            }
        }
    });

    if let Some(info) = &player.media_info {
        let tracks: Vec<String> = info
            .audio_streams()
            .iter()
            .enumerate()
            .map(|(idx, s)| {
                let lang = s
                    .tags
                    .as_ref()
                    .and_then(|t| t.get("language"))
                    .and_then(|v| v.as_str())
                    .unwrap_or("und");
                let codec = s.codec_name.as_deref().unwrap_or("audio");
                format!("Track {} · {} · {}", idx + 1, codec, lang)
            })
            .collect();
        if tracks.len() > 1 {
            ui.add_space(6.0);
            section_label(ui, "Audio track");
            for (idx, label) in tracks.into_iter().enumerate() {
                if option_row(ui, player.active_audio_track == idx, &label).clicked() {
                    player.select_audio_track(idx);
                }
            }
        }
    }

    ui.add_space(6.0);
    section_label(ui, "Audio delay");
    delay_stepper(ui, player.get_audio_delay(), |d| player.set_audio_delay(d));

    ui.add_space(6.0);
    section_label(ui, "Hardware decoding");
    ui.horizontal_wrapped(|ui| {
        for mode in HwAccelMode::ALL {
            if chip(ui, player.hw_accel == mode, mode.short_name())
                .on_hover_text(mode.name())
                .clicked()
            {
                player.set_hw_accel(mode);
            }
        }
    });

    ui.add_space(6.0);
    section_label(ui, "Screenshot format");
    ui.horizontal_wrapped(|ui| {
        for fmt in ScreenshotFormat::ALL {
            if chip(ui, prefs.screenshot_format == fmt, &fmt.extension().to_uppercase()).clicked() {
                prefs.screenshot_format = fmt;
                prefs.save();
            }
        }
    });

    ui.add_space(8.0);
    ui.separator();
    if option_row(ui, false, "Media information").clicked() {
        *show_info = true;
        ui.close();
    }
    if option_row(ui, false, "Preferences…").clicked() {
        *show_preferences = true;
        ui.close();
    }
}

fn subtitles_menu(ui: &mut Ui, player: &mut MediaPlayer) {
    ui.spacing_mut().item_spacing = Vec2::new(4.0, 4.0);
    section_label(ui, "Subtitles");

    if option_row(ui, player.active_subtitle_track.is_none(), "Off").clicked() {
        player.select_subtitle_track(None);
    }
    let mut chosen = None;
    for (idx, sub) in player.subtitles.iter().enumerate() {
        let label = format!("{} · {}", sub.name, sub.language);
        if option_row(ui, player.active_subtitle_track == Some(idx), &label).clicked() {
            chosen = Some(idx);
        }
    }
    if let Some(idx) = chosen {
        player.select_subtitle_track(Some(idx));
    }

    ui.add_space(2.0);
    if option_row(ui, false, "Load subtitle file…").clicked() {
        if let Some(path) = rfd::FileDialog::new()
            .add_filter("Subtitles", &["srt", "vtt", "ass"])
            .pick_file()
        {
            let _ = player.load_external_subtitle(path);
        }
    }

    ui.add_space(6.0);
    section_label(ui, "Delay");
    delay_stepper(ui, player.get_subtitle_delay(), |d| player.set_subtitle_delay(d));
}

fn delay_stepper(ui: &mut Ui, value: i64, mut set: impl FnMut(i64)) {
    ui.horizontal(|ui| {
        if chip(ui, false, "−50").clicked() {
            set(value - 50);
        }
        let (rect, _) = ui.allocate_exact_size(Vec2::new(72.0, 26.0), Sense::hover());
        ui.painter().text(
            rect.center(),
            Align2::CENTER_CENTER,
            format!("{value} ms"),
            Theme::font(13.0),
            if value == 0 { Theme::TEXT_MUTED } else { Theme::TEXT },
        );
        if chip(ui, false, "+50").clicked() {
            set(value + 50);
        }
        if value != 0 && chip(ui, false, "Reset").clicked() {
            set(0);
        }
    });
}

/// Compact pill used for option groups inside popups.
pub fn chip(ui: &mut Ui, selected: bool, text: &str) -> egui::Response {
    let galley = ui.painter().layout_no_wrap(text.to_owned(), Theme::font(12.5), Theme::TEXT);
    let size = galley.size() + Vec2::new(20.0, 10.0);
    let (rect, resp) = ui.allocate_exact_size(size, Sense::click());
    let fill = if selected {
        Theme::ACCENT
    } else if resp.hovered() {
        Theme::white(26)
    } else {
        Theme::white(12)
    };
    ui.painter().rect_filled(rect, Theme::radius_sm(), fill);
    let text_color = if selected || resp.hovered() { Theme::TEXT } else { Theme::TEXT_MUTED };
    ui.painter().galley(rect.center() - galley.size() / 2.0, galley, text_color);
    resp
}

/// Full-width selectable row with a check mark when selected.
pub fn option_row(ui: &mut Ui, selected: bool, text: &str) -> egui::Response {
    let (rect, resp) = ui.allocate_exact_size(Vec2::new(ui.available_width(), 30.0), Sense::click());
    if resp.hovered() {
        ui.painter().rect_filled(rect, Theme::radius_sm(), Theme::white(14));
    }
    let color = if selected || resp.hovered() { Theme::TEXT } else { Theme::TEXT_MUTED };
    ui.painter().text(
        rect.left_center() + Vec2::new(8.0, 0.0),
        Align2::LEFT_CENTER,
        text,
        Theme::font(13.5),
        color,
    );
    if selected {
        let c = rect.right_center() - Vec2::new(14.0, 0.0);
        ui.painter().add(egui::Shape::line(
            vec![c + Vec2::new(-5.0, 0.0), c + Vec2::new(-1.5, 3.5), c + Vec2::new(5.0, -4.0)],
            egui::Stroke::new(1.8, Theme::ACCENT_SOFT),
        ));
    }
    resp
}

pub fn trim_speed(s: f32) -> String {
    let txt = format!("{s:.2}");
    txt.trim_end_matches('0').trim_end_matches('.').to_string()
}

pub fn format_time(seconds: f64) -> String {
    let s = seconds.max(0.0) as u64;
    let hours = s / 3600;
    let mins = (s % 3600) / 60;
    let secs = s % 60;

    if hours > 0 {
        format!("{hours}:{mins:02}:{secs:02}")
    } else {
        format!("{mins}:{secs:02}")
    }
}
