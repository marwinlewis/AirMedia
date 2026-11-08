use super::canvas::AspectRatioMode;
use super::icons::{icon_button, icon_button_ex, primary_circle_button, Icon};
use super::preferences_dialog::PreferencesState;
use super::theme::Theme;
use crate::media::decoder::HwAccelMode;
use crate::media::MediaPlayer;
use crate::playlist::{Playlist, RepeatMode};
use eframe::egui::{
    self, Align2, Popup, PopupCloseBehavior, Pos2, Rect, RectAlign, RichText, Sense, Stroke, Ui,
    Vec2,
};

pub const SPEEDS: [f32; 8] = [0.25, 0.5, 0.75, 1.0, 1.25, 1.5, 2.0, 2.5];

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
    pub show_equalizer: &'a mut bool,
    pub is_fullscreen: bool,
    pub toggle_fullscreen: &'a mut bool,
    pub toast_msg: &'a mut Option<String>,
    pub prefs: &'a mut PreferencesState,
    pub ab_loop: crate::ab_loop::AbLoop,
}

impl ControlsView {
    pub fn new() -> Self {
        Self {
            show_remaining_time: false,
            is_seeking: false,
            seek_preview_secs: None,
        }
    }

    /// Renders the redesigned media player controls:
    /// 1. Thin seek bar with circular scrubber thumb and left/right time indicators
    /// 2. Five-button transport row: Shuffle | Prev | [ Big Play/Pause ] | Next | Repeat
    /// 3. Bottom playlist capsule: "≡ Playlist" on left, "···" More options on right
    pub fn show(
        &mut self,
        ui: &mut Ui,
        area: Rect,
        player: &mut MediaPlayer,
        playlist: &mut Playlist,
        cx: ControlsCtx<'_>,
    ) -> bool {
        let mut hovered = false;
        let pad_x = 24.0;
        let inner_width = (area.width() - pad_x * 2.0).max(200.0);
        let cur_time = player.current_time_secs();
        let dur = player.duration_secs;

        // Container starting near area.min.y
        let mut y = area.min.y;

        // --- 1. Seek bar ---
        let seek_height = 14.0;
        let seek_rect = Rect::from_min_size(Pos2::new(area.min.x + pad_x, y), Vec2::new(inner_width, seek_height));
        let mut seek_ui = ui.new_child(egui::UiBuilder::new().max_rect(seek_rect));
        self.seek_bar(&mut seek_ui, cur_time, dur, player, cx.ab_loop);
        if ui.rect_contains_pointer(seek_rect) {
            hovered = true;
        }
        y += seek_height + 4.0;

        // --- Timestamps row directly below seek bar ---
        let time_rect = Rect::from_min_size(Pos2::new(area.min.x + pad_x, y), Vec2::new(inner_width, 18.0));
        let mut time_ui = ui.new_child(
            egui::UiBuilder::new()
                .max_rect(time_rect)
                .layout(egui::Layout::left_to_right(egui::Align::Center)),
        );

        let shown_time = self.seek_preview_secs.unwrap_or(cur_time);
        let left_txt = format_time(shown_time);
        let right_txt = if self.show_remaining_time && dur > 0.0 {
            format_time((dur - shown_time).max(0.0))
        } else {
            format_time(dur)
        };

        time_ui.label(RichText::new(left_txt).size(12.0).color(Theme::TEXT_MUTED));
        time_ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            let resp = ui.add(
                egui::Label::new(RichText::new(right_txt).size(12.0).color(Theme::TEXT_MUTED))
                    .sense(Sense::click())
                    .selectable(false),
            ).on_hover_text("Toggle remaining time");
            if resp.clicked() {
                self.show_remaining_time = !self.show_remaining_time;
            }
        });
        y += 24.0;

        // --- 2. Five-button Transport Controls ---
        let controls_height = 64.0;
        let row_rect = Rect::from_min_size(Pos2::new(area.min.x + pad_x, y), Vec2::new(inner_width, controls_height));
        let mut row = ui.new_child(
            egui::UiBuilder::new()
                .max_rect(row_rect)
                .layout(egui::Layout::left_to_right(egui::Align::Center)),
        );
        if ui.rect_contains_pointer(row_rect) {
            hovered = true;
        }

        let slot_w = inner_width / 5.0;

        // 1: Shuffle
        let (r1, _) = row.allocate_exact_size(Vec2::new(slot_w, controls_height), Sense::hover());
        let mut u1 = row.new_child(egui::UiBuilder::new().max_rect(r1).layout(egui::Layout::centered_and_justified(egui::Direction::LeftToRight)));
        if icon_button_ex(&mut u1, Icon::Shuffle, 34.0, playlist.is_shuffle)
            .on_hover_text(if playlist.is_shuffle { "Shuffle on" } else { "Shuffle off" })
            .clicked()
        {
            playlist.is_shuffle = !playlist.is_shuffle;
        }

        // 2: Previous
        let (r2, _) = row.allocate_exact_size(Vec2::new(slot_w, controls_height), Sense::hover());
        let mut u2 = row.new_child(egui::UiBuilder::new().max_rect(r2).layout(egui::Layout::centered_and_justified(egui::Direction::LeftToRight)));
        if icon_button(&mut u2, Icon::Prev, 38.0).on_hover_text("Previous track (P)").clicked() {
            if let Some(item) = playlist.prev() {
                let _ = player.load_file(&item.path);
            }
        }

        // 3: Play / Pause Center (Prominent Cream Circle)
        let (r3, _) = row.allocate_exact_size(Vec2::new(slot_w, controls_height), Sense::hover());
        let mut u3 = row.new_child(egui::UiBuilder::new().max_rect(r3).layout(egui::Layout::centered_and_justified(egui::Direction::LeftToRight)));
        let play_icon = if player.is_playing { Icon::Pause } else { Icon::Play };
        if primary_circle_button(&mut u3, play_icon, 58.0, Theme::ACCENT, Theme::ACCENT_DARK)
            .on_hover_text(if player.is_playing { "Pause (Space)" } else { "Play (Space)" })
            .clicked()
        {
            player.toggle_play();
        }

        // 4: Next
        let (r4, _) = row.allocate_exact_size(Vec2::new(slot_w, controls_height), Sense::hover());
        let mut u4 = row.new_child(egui::UiBuilder::new().max_rect(r4).layout(egui::Layout::centered_and_justified(egui::Direction::LeftToRight)));
        if icon_button(&mut u4, Icon::Next, 38.0).on_hover_text("Next track (N)").clicked() {
            if let Some(item) = playlist.next() {
                let _ = player.load_file(&item.path);
            }
        }

        // 5: Repeat
        let (r5, _) = row.allocate_exact_size(Vec2::new(slot_w, controls_height), Sense::hover());
        let mut u5 = row.new_child(egui::UiBuilder::new().max_rect(r5).layout(egui::Layout::centered_and_justified(egui::Direction::LeftToRight)));
        let repeat_on = playlist.repeat_mode != RepeatMode::Off;
        let repeat_resp = icon_button_ex(&mut u5, Icon::Repeat, 34.0, repeat_on).on_hover_text(playlist.repeat_mode.name());
        if playlist.repeat_mode == RepeatMode::One {
            let badge = repeat_resp.rect.right_top() + Vec2::new(-6.0, 7.0);
            u5.painter().text(badge, Align2::CENTER_CENTER, "1", Theme::font_semibold(9.0), Theme::ACCENT);
        }
        if repeat_resp.clicked() {
            playlist.repeat_mode = playlist.repeat_mode.cycle();
        }
        y += controls_height + 14.0;

        // --- 3. Bottom Playlist Capsule Bar ---
        let bar_height = 48.0;
        let capsule_rect = Rect::from_min_size(Pos2::new(area.min.x + pad_x, y), Vec2::new(inner_width, bar_height));
        ui.painter().rect(
            capsule_rect,
            Theme::radius_md(),
            Theme::CAPSULE_BG,
            Stroke::new(1.0, Theme::BORDER),
            Theme::stroke_kind(),
        );

        let left_pill_rect = Rect::from_min_size(capsule_rect.min, Vec2::new(inner_width - 48.0, bar_height));
        let pill_resp = ui.allocate_rect(left_pill_rect, Sense::click());
        if pill_resp.hovered() {
            ui.painter().rect_filled(left_pill_rect, Theme::radius_md(), Theme::white(8));
        }
        if pill_resp.clicked() {
            *cx.show_playlist = !*cx.show_playlist;
        }

        let mut capsule_ui = ui.new_child(
            egui::UiBuilder::new()
                .max_rect(capsule_rect)
                .layout(egui::Layout::left_to_right(egui::Align::Center)),
        );

        let mut left_ui = capsule_ui.new_child(
            egui::UiBuilder::new()
                .max_rect(left_pill_rect)
                .layout(egui::Layout::left_to_right(egui::Align::Center)),
        );
        left_ui.add_space(14.0);
        let (icon_r, _) = left_ui.allocate_exact_size(Vec2::splat(18.0), Sense::hover());
        Icon::Playlist.paint(left_ui.painter(), icon_r, Theme::TEXT);
        left_ui.add_space(8.0);
        left_ui.label(RichText::new("Playlist").font(Theme::font_semibold(15.0)).color(Theme::TEXT));
        if !playlist.items.is_empty() {
            left_ui.add_space(4.0);
            left_ui.label(RichText::new(format!("({})", playlist.items.len())).size(13.0).color(Theme::TEXT_FAINT));
        }

        // Right portion: "···" More options
        capsule_ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            ui.add_space(8.0);
            let more_resp = icon_button(ui, Icon::More, 34.0).on_hover_text("More Options");
            popup_above(&more_resp).width(280.0).show(|ui| {
                more_menu(ui, player, cx.aspect_mode, cx.show_info, cx.show_preferences, cx.show_equalizer, cx.prefs, cx.toggle_fullscreen, cx.toast_msg);
            });
        });

        hovered || self.is_seeking
    }

    fn seek_bar(&mut self, ui: &mut Ui, cur_time: f64, total: f64, player: &mut MediaPlayer, ab: crate::ab_loop::AbLoop) {
        let rect = ui.max_rect();
        let resp = ui.allocate_rect(rect, Sense::click_and_drag());
        let painter = ui.painter();

        let hot = resp.hovered() || resp.dragged();
        let rail_h = 3.5;
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

        // Background track (dark charcoal)
        painter.rect_filled(rail, r, Theme::RAIL_BG);

        // Hover highlight
        if let (Some(f), true) = (pointer_frac, hot) {
            let hover_rect = Rect::from_min_max(rail.min, Pos2::new(rail.min.x + rail.width() * f, rail.max.y));
            painter.rect_filled(hover_rect, r, Theme::white(30));
        }

        // Played portion (cream)
        let fill = Rect::from_min_max(rail.min, Pos2::new(rail.min.x + rail.width() * progress, rail.max.y));
        painter.rect_filled(fill, r, Theme::ACCENT);

        // A-B loop: shaded span plus a tick at each point.
        if total > 0.0 {
            let x_at = |t: f64| rail.min.x + rail.width() * (t / total).clamp(0.0, 1.0) as f32;
            if let (Some(a), Some(b)) = (ab.a, ab.b) {
                let span = Rect::from_min_max(Pos2::new(x_at(a), rail.min.y - 2.0), Pos2::new(x_at(b), rail.max.y + 2.0));
                painter.rect_filled(span, r, Theme::ACCENT_SOFT.gamma_multiply(0.35));
            }
            for p in [ab.a, ab.b].into_iter().flatten() {
                let x = x_at(p);
                painter.line_segment(
                    [Pos2::new(x, rail.min.y - 5.0), Pos2::new(x, rail.max.y + 5.0)],
                    egui::Stroke::new(2.0, Theme::ACCENT_SOFT),
                );
            }
        }

        // Scrubber thumb circle at current position
        let knob_center = Pos2::new(fill.max.x, rail.center().y);
        let knob_radius = if hot { 6.0 } else { 5.0 };
        painter.circle_filled(knob_center, knob_radius, Theme::ACCENT);

        // Floating time bubble when hovering
        if let (Some(f), true, true) = (pointer_frac, hot, total > 0.0) {
            let secs = f as f64 * total;
            let text = format_time(secs);
            let galley = painter.layout_no_wrap(text, Theme::font(12.0), Theme::TEXT);
            let size = galley.size() + Vec2::new(16.0, 8.0);
            let x = (rail.min.x + rail.width() * f).clamp(rail.min.x + size.x / 2.0, rail.max.x - size.x / 2.0);
            let bubble = Rect::from_center_size(Pos2::new(x, rail.min.y - 18.0), size);
            painter.rect_filled(bubble, Theme::radius_sm(), Theme::black(230));
            painter.galley(bubble.center() - galley.size() / 2.0, galley, Theme::TEXT);
        }

        if hot {
            ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand);
        }
    }
}

/// Compact custom volume slider (0–150%). Returns true when the value changed.
pub fn volume_slider(ui: &mut Ui, value: &mut f32) -> bool {
    let (rect, resp) = ui.allocate_exact_size(Vec2::new(100.0, 26.0), Sense::click_and_drag());
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
    painter.rect_filled(rail, r, Theme::RAIL_BG);
    let frac = (*value / max).clamp(0.0, 1.0);
    let fill = Rect::from_min_max(rail.min, Pos2::new(rail.min.x + rail.width() * frac, rail.max.y));
    let fill_color = if *value > 1.0 { Theme::ACCENT_SOFT } else { Theme::ACCENT };
    painter.rect_filled(fill, r, fill_color);
    painter.circle_filled(Pos2::new(fill.max.x, rail.center().y), 4.5 + 1.5 * t, Theme::ACCENT);

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

fn more_menu(
    ui: &mut Ui,
    player: &mut MediaPlayer,
    aspect: &mut AspectRatioMode,
    show_info: &mut bool,
    show_preferences: &mut bool,
    show_equalizer: &mut bool,
    prefs: &mut PreferencesState,
    toggle_fullscreen: &mut bool,
    toast_msg: &mut Option<String>,
) {
    ui.spacing_mut().item_spacing = Vec2::new(4.0, 4.0);

    // Fullscreen action
    if option_row(ui, false, "Toggle Fullscreen (F)").clicked() {
        *toggle_fullscreen = true;
        ui.close();
    }

    // Volume & Mute
    section_label(ui, "Volume");
    ui.horizontal(|ui| {
        let vol = player.get_volume();
        let muted = player.is_muted();
        let vol_icon = if muted || vol <= 0.001 {
            Icon::VolumeMute
        } else if vol < 0.5 {
            Icon::VolumeLow
        } else {
            Icon::VolumeHigh
        };
        if icon_button(ui, vol_icon, 28.0).on_hover_text("Mute (M)").clicked() {
            player.toggle_mute();
        }
        let mut shown_vol = if muted { 0.0 } else { vol };
        if volume_slider(ui, &mut shown_vol) {
            if muted {
                player.toggle_mute();
            }
            player.set_volume(shown_vol);
        }
    });

    ui.add_space(4.0);
    section_label(ui, "Playback Speed");
    ui.horizontal_wrapped(|ui| {
        for s in SPEEDS {
            let selected = (player.playback_speed - s).abs() < 0.01;
            if chip(ui, selected, &format!("{}×", trim_speed(s))).clicked() {
                player.set_speed(s);
            }
        }
    });

    ui.add_space(4.0);
    section_label(ui, "Equalizer & Tools");
    if option_row(ui, false, "Equalizer (Ctrl+E)").clicked() {
        *show_equalizer = true;
        ui.close();
    }
    if option_row(ui, false, "Take Screenshot (Shift+S)").clicked() {
        match player.take_screenshot(&prefs.screenshot_dir, prefs.screenshot_format) {
            Ok(path) => *toast_msg = Some(format!("Screenshot saved: {}", path.display())),
            Err(e) => *toast_msg = Some(format!("Screenshot failed: {e}")),
        }
        ui.close();
    }

    ui.add_space(4.0);
    section_label(ui, "Video & Display");
    ui.horizontal_wrapped(|ui| {
        for mode in AspectRatioMode::ALL {
            let selected = *aspect == mode;
            if chip(ui, selected, mode.short_name())
                .on_hover_text(mode.name())
                .clicked()
            {
                *aspect = mode;
            }
        }
    });

    ui.add_space(4.0);
    section_label(ui, "Hardware Decoder");
    ui.horizontal_wrapped(|ui| {
        for mode in HwAccelMode::ALL {
            let selected = player.hw_accel == mode;
            if chip(ui, selected, mode.short_name())
                .on_hover_text(mode.name())
                .clicked()
            {
                player.set_hw_accel(mode);
            }
        }
    });

    ui.add_space(6.0);
    ui.separator();
    if option_row(ui, false, "Media & Codec Information (I)").clicked() {
        *show_info = true;
        ui.close();
    }
    if option_row(ui, false, "Preferences… (Ctrl+P)").clicked() {
        *show_preferences = true;
        ui.close();
    }
}

/// Compact pill used for option groups inside popups.
pub fn chip(ui: &mut Ui, selected: bool, text: &str) -> egui::Response {
    let galley = ui.painter().layout_no_wrap(text.to_owned(), Theme::font(12.5), Theme::TEXT);
    let size = galley.size() + Vec2::new(18.0, 8.0);
    let (rect, resp) = ui.allocate_exact_size(size, Sense::click());
    let fill = if selected {
        Theme::ACCENT
    } else if resp.hovered() {
        Theme::white(26)
    } else {
        Theme::white(12)
    };
    ui.painter().rect_filled(rect, Theme::radius_sm(), fill);
    let text_color = if selected { Theme::ACCENT_DARK } else if resp.hovered() { Theme::TEXT } else { Theme::TEXT_MUTED };
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
