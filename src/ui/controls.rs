//! The player's fade-away controls: back button and title on top, seek bar and transport at the
//! bottom, and the big icon that flashes on play/pause.

use super::canvas::AspectRatioMode;
use super::icons::{pill_button, Icon, PillStyle};
use super::preferences_dialog::PreferencesState;
use super::theme::Theme;
use crate::media::decoder::HwAccelMode;
use crate::media::MediaPlayer;
use crate::playlist::RepeatMode;
use eframe::egui::{
    self, text::LayoutJob, text::TextWrapping, Align2, Popup, PopupCloseBehavior, Pos2, Rect, RectAlign, Response, RichText, Sense, Ui,
    UiBuilder, Vec2,
};
use std::time::Instant;

pub const SPEEDS: [f32; 8] = [0.25, 0.5, 0.75, 1.0, 1.25, 1.5, 2.0, 2.5];

/// Height the bottom controls take up, for lifting subtitles and laying out the audio screen.
pub const BOTTOM_CONTROLS_H: f32 = 104.0;
const TOP_BAR_H: f32 = 84.0;
const FLASH_SECS: f32 = 0.55;
/// How long before the end the "next" shortcut appears.
const UP_NEXT_WINDOW_SECS: f64 = 15.0;

pub struct ControlsView {
    pub show_remaining_time: bool,
    pub is_seeking: bool,
    pub seek_preview_secs: Option<f64>,
    was_playing: bool,
    flash: Option<(Icon, Instant)>,
}

/// Everything the controls can change outside of the player itself.
pub struct ControlsCtx<'a> {
    pub title: &'a str,
    pub subtitle: &'a str,
    /// Title of what plays next, when that is known.
    pub up_next: Option<&'a str>,
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
    pub cycle_ab_loop: &'a mut bool,
    pub open_jump: &'a mut bool,
    pub go_back: &'a mut bool,
    pub play_next: &'a mut bool,
    pub play_prev: &'a mut bool,
    pub shuffle: &'a mut bool,
    pub repeat: &'a mut RepeatMode,
    pub has_queue: bool,
    pub audio_only: bool,
}

pub struct ControlsOutput {
    /// The pointer is over the controls, so they should stay up.
    pub hovered: bool,
    /// The app-menu button, for the app to attach its menu to.
    pub menu: Response,
}

impl ControlsView {
    pub fn new() -> Self {
        Self {
            show_remaining_time: false,
            is_seeking: false,
            seek_preview_secs: None,
            was_playing: false,
            flash: None,
        }
    }

    /// Paints the controls over `area`, faded by `opacity` (0 = hidden, nothing interactive).
    pub fn show(&mut self, ui: &mut Ui, area: Rect, opacity: f32, player: &mut MediaPlayer, mut cx: ControlsCtx<'_>) -> Option<ControlsOutput> {
        self.up_next_shortcut(ui, area, opacity, player, &mut cx);
        if opacity <= 0.001 {
            return None;
        }

        let mut layer = ui.new_child(UiBuilder::new().id_salt("player_controls").max_rect(area));
        layer.multiply_opacity(opacity);
        layer.shrink_clip_rect(area);
        layer.spacing_mut().item_spacing = Vec2::new(2.0, 0.0);

        let top = Rect::from_min_size(area.min, Vec2::new(area.width(), TOP_BAR_H));
        let bottom = Rect::from_min_max(Pos2::new(area.min.x, area.max.y - BOTTOM_CONTROLS_H - 44.0), area.max);
        let painter = layer.painter().clone();
        Theme::paint_vertical_gradient(&painter, top.with_max_y(top.max.y + 36.0), Theme::black(225), Theme::black(0));
        Theme::paint_vertical_gradient(&painter, bottom, Theme::black(0), Theme::black(225));

        let pad = (area.width() * 0.025).clamp(10.0, 28.0);

        // --- Top: back, title, menu ---
        let row = Rect::from_min_size(area.min + Vec2::new(pad, 12.0), Vec2::new(area.width() - pad * 2.0, 44.0));
        let mut top_ui = layer.new_child(UiBuilder::new().id_salt("player_top").max_rect(row).layout(egui::Layout::left_to_right(egui::Align::Center)));
        if icon_button_big(&mut top_ui, Icon::ArrowLeft, 44.0).on_hover_text("Back to library (Esc)").clicked() {
            *cx.go_back = true;
        }
        let menu = {
            let mut right = top_ui.new_child(UiBuilder::new().id_salt("player_top_right").max_rect(row).layout(egui::Layout::right_to_left(egui::Align::Center)));
            icon_button_big(&mut right, Icon::Menu, 40.0).on_hover_text("Menu")
        };
        let text_max = (row.width() - 44.0 - 40.0 - 28.0).max(60.0);
        let text_x = row.min.x + 44.0 + 12.0;
        let text_line = |text: &str, font, color| {
            let mut job = LayoutJob::simple_singleline(text.to_owned(), font, color);
            job.wrap = TextWrapping::truncate_at_width(text_max);
            painter.layout_job(job)
        };
        if !cx.audio_only {
            if cx.subtitle.is_empty() {
                painter.galley(Pos2::new(text_x, row.center().y - 13.0), text_line(cx.title, Theme::font_bold(18.5), Theme::TEXT), Theme::TEXT);
            } else {
                painter.galley(Pos2::new(text_x, row.min.y), text_line(cx.title, Theme::font_bold(17.5), Theme::TEXT), Theme::TEXT);
                painter.galley(Pos2::new(text_x, row.min.y + 25.0), text_line(cx.subtitle, Theme::font(13.0), Theme::TEXT_MUTED), Theme::TEXT_MUTED);
            }
        }

        // --- Bottom: seek bar with remaining time, then the transport row ---
        let cur_time = player.current_time_secs();
        let dur = player.duration_secs;
        let seek_rect = Rect::from_min_size(Pos2::new(area.min.x + pad, area.max.y - 92.0), Vec2::new(area.width() - pad * 2.0, 20.0));
        let mut seek_ui = layer.new_child(UiBuilder::new().id_salt("player_seek").max_rect(seek_rect));
        self.seek_bar(&mut seek_ui, cur_time, dur, player, cx.ab_loop);

        let row = Rect::from_min_size(Pos2::new(area.min.x + pad, area.max.y - 68.0), Vec2::new(area.width() - pad * 2.0, 56.0));
        let w = row.width();
        let wide = w >= 900.0;
        let medium = w >= 620.0;

        let mut left = layer.new_child(UiBuilder::new().id_salt("player_left").max_rect(row).layout(egui::Layout::left_to_right(egui::Align::Center)));
        left.spacing_mut().item_spacing = Vec2::new(4.0, 0.0);

        let play_icon = if player.is_playing { Icon::Pause } else { Icon::Play };
        if icon_button_big(&mut left, play_icon, 50.0)
            .on_hover_text(if player.is_playing { "Pause (Space)" } else { "Play (Space)" })
            .clicked()
        {
            player.toggle_play();
        }
        if medium {
            if icon_button_big(&mut left, Icon::Back10, 46.0).on_hover_text("Back 10 seconds").clicked() {
                player.seek(cur_time - 10.0);
            }
            if icon_button_big(&mut left, Icon::Forward10, 46.0).on_hover_text("Forward 10 seconds").clicked() {
                player.seek(cur_time + 10.0);
            }
        }

        let vol = player.get_volume();
        let muted = player.is_muted();
        let vol_icon = if muted || vol <= 0.001 {
            Icon::VolumeMute
        } else if vol < 0.5 {
            Icon::VolumeLow
        } else {
            Icon::VolumeHigh
        };
        if icon_button_big(&mut left, vol_icon, 46.0).on_hover_text(if muted { "Unmute (M)" } else { "Mute (M)" }).clicked() {
            player.toggle_mute();
        }
        if wide {
            let mut shown = if muted { 0.0 } else { vol };
            if volume_slider(&mut left, &mut shown) {
                if muted {
                    player.toggle_mute();
                }
                player.set_volume(shown);
            }
            left.add_space(14.0);
        }

        // Elapsed / total, or remaining; click to switch.
        let shown_time = self.seek_preview_secs.unwrap_or(cur_time);
        let right_txt = if self.show_remaining_time && dur > 0.0 {
            format!("\u{2212}{}", format_time((dur - shown_time).max(0.0)))
        } else {
            format_time(dur)
        };
        let time_txt = format!("{} / {right_txt}", format_time(shown_time));
        let time_resp = left
            .add(
                egui::Label::new(RichText::new(time_txt).size(13.5).color(Theme::TEXT))
                    .sense(Sense::click())
                    .selectable(false),
            )
            .on_hover_text("Switch between total and remaining time");
        if time_resp.clicked() {
            self.show_remaining_time = !self.show_remaining_time;
        }

        let mut right = layer.new_child(UiBuilder::new().id_salt("player_right").max_rect(row).layout(egui::Layout::right_to_left(egui::Align::Center)));
        right.spacing_mut().item_spacing = Vec2::new(4.0, 0.0);

        let fs_icon = if cx.is_fullscreen { Icon::ExitFullscreen } else { Icon::Fullscreen };
        if icon_button_big(&mut right, fs_icon, 46.0).on_hover_text("Fullscreen (F)").clicked() {
            *cx.toggle_fullscreen = true;
        }

        let settings_resp = icon_button_big(&mut right, Icon::Settings, 46.0).on_hover_text("Settings");
        popup_above(&settings_resp).width(300.0).show(|ui| {
            settings_menu(ui, player, cx.aspect_mode, cx.show_info, cx.show_preferences, cx.show_equalizer, cx.prefs, cx.toast_msg, cx.ab_loop, cx.cycle_ab_loop, cx.open_jump);
        });

        let tracks_resp = icon_button_big(&mut right, Icon::Subtitles, 46.0).on_hover_text("Audio & subtitles");
        popup_above(&tracks_resp).width(if area.width() < 480.0 { 280.0 } else { 440.0 }).show(|ui| {
            tracks_menu(ui, player);
        });

        if medium {
            let speed = player.playback_speed;
            let speed_resp = icon_button_ex_big(&mut right, Icon::Speed, 46.0, (speed - 1.0).abs() > 0.01)
                .on_hover_text(format!("Playback speed ({}\u{00d7})", trim_speed(speed)));
            popup_above(&speed_resp).width(150.0).show(|ui| {
                section_label(ui, "Playback speed");
                for s in SPEEDS {
                    let selected = (player.playback_speed - s).abs() < 0.01;
                    let label = if s == 1.0 { "1\u{00d7} (Normal)".to_string() } else { format!("{}\u{00d7}", trim_speed(s)) };
                    if option_row(ui, selected, &label).clicked() {
                        player.set_speed(s);
                        ui.close();
                    }
                }
            });
        }

        let playlist_resp = icon_button_ex_big(&mut right, Icon::Playlist, 46.0, *cx.show_playlist).on_hover_text("Playlist (L)");
        if playlist_resp.clicked() {
            *cx.show_playlist = !*cx.show_playlist;
        }

        if medium {
            let repeat_on = *cx.repeat != RepeatMode::Off;
            let repeat_resp = icon_button_ex_big(&mut right, Icon::Repeat, 40.0, repeat_on).on_hover_text(cx.repeat.name());
            if *cx.repeat == RepeatMode::One {
                let badge = repeat_resp.rect.right_top() + Vec2::new(-8.0, 10.0);
                right.painter().text(badge, Align2::CENTER_CENTER, "1", Theme::font_bold(10.0), Theme::ACCENT);
            }
            if repeat_resp.clicked() {
                *cx.repeat = cx.repeat.cycle();
            }
            if icon_button_ex_big(&mut right, Icon::Shuffle, 40.0, *cx.shuffle)
                .on_hover_text(if *cx.shuffle { "Shuffle on" } else { "Shuffle off" })
                .clicked()
            {
                *cx.shuffle = !*cx.shuffle;
            }
        }

        if cx.has_queue {
            if icon_button_big(&mut right, Icon::Next, 46.0).on_hover_text("Next (N)").clicked() {
                *cx.play_next = true;
            }
            if medium && icon_button_big(&mut right, Icon::Prev, 46.0).on_hover_text("Previous (P)").clicked() {
                *cx.play_prev = true;
            }
        }

        let hovered = layer.rect_contains_pointer(top) || layer.rect_contains_pointer(Rect::from_min_max(Pos2::new(area.min.x, area.max.y - BOTTOM_CONTROLS_H), area.max));
        Some(ControlsOutput { hovered: hovered || self.is_seeking, menu })
    }

    /// Button that appears near the end of an item and jumps straight to the next one.
    fn up_next_shortcut(&mut self, ui: &mut Ui, area: Rect, opacity: f32, player: &mut MediaPlayer, cx: &mut ControlsCtx<'_>) {
        let Some(next) = cx.up_next else { return };
        let dur = player.duration_secs;
        let left = dur - player.current_time_secs();
        if dur <= UP_NEXT_WINDOW_SECS * 2.0 || left > UP_NEXT_WINDOW_SECS || left <= 0.0 {
            return;
        }
        let label: String = next.chars().take(34).collect();
        let label = if next.chars().count() > 34 { format!("Next: {label}\u{2026}") } else { format!("Next: {label}") };
        // Sits above the controls when they're showing, in the corner when they're not.
        let lift = if opacity > 0.5 { BOTTOM_CONTROLS_H + 16.0 } else { 32.0 };
        let rect = Rect::from_min_size(Pos2::new(area.min.x, area.max.y - lift - 44.0), Vec2::new(area.width() - 28.0, 44.0));
        let mut host = ui.new_child(UiBuilder::new().id_salt("up_next").max_rect(rect).layout(egui::Layout::right_to_left(egui::Align::Center)));
        if pill_button(&mut host, Some(Icon::Next), &label, PillStyle::Primary, 42.0).clicked() {
            *cx.play_next = true;
        }
    }

    /// The big icon that pulses in the middle when playback pauses or resumes.
    pub fn paint_flash(&mut self, ui: &Ui, area: Rect, player: &MediaPlayer) {
        if player.is_playing != self.was_playing {
            self.was_playing = player.is_playing;
            let icon = if player.is_playing { Icon::Play } else { Icon::Pause };
            self.flash = Some((icon, Instant::now()));
        }
        let Some((icon, at)) = self.flash else { return };
        let t = at.elapsed().as_secs_f32() / FLASH_SECS;
        if t >= 1.0 {
            self.flash = None;
            return;
        }
        let size = 74.0 + 26.0 * t;
        let alpha = (1.0 - t) * 0.85;
        let c = area.center();
        let painter = ui.painter();
        painter.circle_filled(c, size / 2.0, Theme::black((170.0 * alpha) as u8));
        let color = egui::Color32::from_white_alpha((255.0 * alpha) as u8);
        icon.paint(painter, Rect::from_center_size(c, Vec2::splat(size * 0.48)), color);
        ui.ctx().request_repaint();
    }

    fn seek_bar(&mut self, ui: &mut Ui, cur_time: f64, total: f64, player: &mut MediaPlayer, ab: crate::ab_loop::AbLoop) {
        let rect = ui.max_rect();
        let resp = ui.allocate_rect(rect, Sense::click_and_drag());
        let painter = ui.painter();

        let hot = resp.hovered() || resp.dragged();
        let grow = ui.ctx().animate_bool_with_time(resp.id, hot, 0.12);
        let rail_h = 4.0 + 2.5 * grow;
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

        painter.rect_filled(rail, r, Theme::white(70));

        if let (Some(f), true) = (pointer_frac, hot) {
            let hover_rect = Rect::from_min_max(rail.min, Pos2::new(rail.min.x + rail.width() * f, rail.max.y));
            painter.rect_filled(hover_rect, r, Theme::white(60));
        }

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

        let knob_center = Pos2::new(fill.max.x, rail.center().y);
        painter.circle_filled(knob_center, 5.5 + 2.5 * grow, Theme::ACCENT);

        // Floating time bubble when hovering
        if let (Some(f), true, true) = (pointer_frac, hot, total > 0.0) {
            let secs = f as f64 * total;
            let text = format_time(secs);
            let galley = painter.layout_no_wrap(text, Theme::font_semibold(12.5), Theme::TEXT);
            let size = galley.size() + Vec2::new(16.0, 8.0);
            let x = (rail.min.x + rail.width() * f).clamp(rail.min.x + size.x / 2.0, rail.max.x - size.x / 2.0);
            let bubble = Rect::from_center_size(Pos2::new(x, rail.min.y - 20.0), size);
            painter.rect_filled(bubble, Theme::radius_sm(), Theme::black(230));
            painter.galley(bubble.center() - galley.size() / 2.0, galley, Theme::TEXT);
        }

        if hot {
            ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand);
        }
    }
}

/// Bigger icons inside the same hit target: player controls are read from a distance.
pub fn icon_button_big(ui: &mut Ui, icon: Icon, size: f32) -> Response {
    icon_button_ex_big(ui, icon, size, false)
}

pub fn icon_button_ex_big(ui: &mut Ui, icon: Icon, size: f32, active: bool) -> Response {
    let (rect, resp) = ui.allocate_exact_size(Vec2::splat(size), Sense::click());
    if ui.is_rect_visible(rect) {
        let hover = ui.ctx().animate_bool_with_time(resp.id, resp.hovered(), 0.12);
        let painter = ui.painter();
        if hover > 0.0 {
            painter.circle_filled(rect.center(), size * 0.5, Theme::white((30.0 * hover) as u8));
        }
        let color = if active { Theme::ACCENT } else { Theme::TEXT };
        let scale = 0.58 * if resp.is_pointer_button_down_on() { 0.92 } else { 1.0 + 0.06 * hover };
        icon.paint(painter, Rect::from_center_size(rect.center(), Vec2::splat(size * scale)), color);
    }
    resp
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
    painter.rect_filled(rail, r, Theme::white(70));
    let frac = (*value / max).clamp(0.0, 1.0);
    let fill = Rect::from_min_max(rail.min, Pos2::new(rail.min.x + rail.width() * frac, rail.max.y));
    let fill_color = if *value > 1.0 { Theme::ACCENT } else { Theme::TEXT };
    painter.rect_filled(fill, r, fill_color);
    painter.circle_filled(Pos2::new(fill.max.x, rail.center().y), 5.0 + 1.5 * t, fill_color);
    // Marks 100%: everything to the right is boost.
    let hundred = rail.min.x + rail.width() / max;
    painter.line_segment(
        [Pos2::new(hundred, rail.min.y - 3.0), Pos2::new(hundred, rail.max.y + 3.0)],
        egui::Stroke::new(1.0, Theme::white(110)),
    );

    resp.on_hover_text(format!("Volume {}%", (*value * 100.0).round() as u32));
    changed
}

fn popup_above(resp: &Response) -> Popup<'_> {
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

/// Audio tracks on the left, subtitles on the right, with the sync offsets under each.
fn tracks_menu(ui: &mut Ui, player: &mut MediaPlayer) {
    ui.columns(2, |cols| {
        let [audio, subs] = &mut cols[..] else { return };

        section_label(audio, "Audio");
        let mut chosen = None;
        if let Some(info) = &player.media_info {
            let streams = info.audio_streams();
            if streams.is_empty() {
                audio.label(RichText::new("No audio tracks").color(Theme::TEXT_FAINT));
            }
            for (idx, a) in streams.iter().enumerate() {
                let tag = |key: &str| a.tags.as_ref().and_then(|t| t.get(key)).and_then(|v| v.as_str());
                let name = tag("title").or(tag("language")).unwrap_or("Track");
                let codec = a.codec_name.as_deref().unwrap_or("audio");
                if option_row(audio, player.active_audio_track == idx, &format!("{name} \u{00b7} {codec}")).clicked() {
                    chosen = Some(idx);
                }
            }
        }
        if let Some(idx) = chosen {
            player.select_audio_track(idx);
        }
        audio.add_space(6.0);
        let delay = player.get_audio_delay();
        if let Some(next) = delay_stepper(audio, "Delay", delay) {
            player.set_audio_delay(next);
        }

        section_label(subs, "Subtitles");
        if option_row(subs, player.active_subtitle_track.is_none(), "Off").clicked() {
            player.select_subtitle_track(None);
        }
        let mut chosen = None;
        for (idx, sub) in player.subtitles.iter().enumerate() {
            if option_row(subs, player.active_subtitle_track == Some(idx), &format!("{} \u{00b7} {}", sub.name, sub.language)).clicked() {
                chosen = Some(idx);
            }
        }
        if let Some(idx) = chosen {
            player.select_subtitle_track(Some(idx));
        }
        if option_row(subs, false, "Load file\u{2026}").clicked() {
            if let Some(path) = crate::ui::FileDialog::new()
                .add_filter("Subtitles", &crate::media::subtitle::SUBTITLE_EXTENSIONS)
                .pick_file()
            {
                let _ = player.load_external_subtitle(path);
            }
        }
        subs.add_space(6.0);
        let delay = player.get_subtitle_delay();
        if let Some(next) = delay_stepper(subs, "Delay", delay) {
            player.set_subtitle_delay(next);
        }
    });
}

/// `Delay  − 0 ms +`: returns the new value when a step button was pressed.
fn delay_stepper(ui: &mut Ui, label: &str, delay_ms: i64) -> Option<i64> {
    let mut next = None;
    ui.horizontal(|ui| {
        ui.spacing_mut().item_spacing.x = 4.0;
        ui.label(RichText::new(label).size(12.5).color(Theme::TEXT_MUTED));
        if chip(ui, false, "\u{2212}").on_hover_text("100 ms earlier").clicked() {
            next = Some(delay_ms - 100);
        }
        ui.label(RichText::new(format!("{delay_ms} ms")).size(12.5).color(Theme::TEXT));
        if chip(ui, false, "+").on_hover_text("100 ms later").clicked() {
            next = Some(delay_ms + 100);
        }
    });
    next
}

#[allow(clippy::too_many_arguments)]
fn settings_menu(
    ui: &mut Ui,
    player: &mut MediaPlayer,
    aspect: &mut AspectRatioMode,
    show_info: &mut bool,
    show_preferences: &mut bool,
    show_equalizer: &mut bool,
    prefs: &mut PreferencesState,
    toast_msg: &mut Option<String>,
    ab: crate::ab_loop::AbLoop,
    cycle_ab_loop: &mut bool,
    open_jump: &mut bool,
) {
    ui.spacing_mut().item_spacing = Vec2::new(4.0, 4.0);

    section_label(ui, "Playback speed");
    ui.horizontal_wrapped(|ui| {
        for s in SPEEDS {
            let selected = (player.playback_speed - s).abs() < 0.01;
            if chip(ui, selected, &format!("{}\u{00d7}", trim_speed(s))).clicked() {
                player.set_speed(s);
            }
        }
    });

    ui.add_space(4.0);
    section_label(ui, "Picture");
    ui.horizontal_wrapped(|ui| {
        for mode in AspectRatioMode::ALL {
            let selected = *aspect == mode;
            if chip(ui, selected, mode.short_name()).on_hover_text(mode.name()).clicked() {
                *aspect = mode;
            }
        }
    });

    ui.add_space(4.0);
    section_label(ui, "Hardware decoder");
    ui.horizontal_wrapped(|ui| {
        for mode in HwAccelMode::ALL {
            let selected = player.hw_accel == mode;
            if chip(ui, selected, mode.short_name()).on_hover_text(mode.name()).clicked() {
                player.set_hw_accel(mode);
            }
        }
    });

    ui.add_space(6.0);
    ui.separator();
    if menu_row(ui, Icon::Settings, "Equalizer & video filters", "Ctrl+E").clicked() {
        *show_equalizer = true;
        ui.close();
    }
    if menu_row(ui, Icon::Camera, "Take screenshot", "Shift+S").clicked() {
        match player.take_screenshot(&prefs.screenshot_dir, prefs.screenshot_format) {
            Ok(path) => *toast_msg = Some(format!("Screenshot saved: {}", path.display())),
            Err(e) => *toast_msg = Some(format!("Screenshot failed: {e}")),
        }
        ui.close();
    }
    if menu_row(ui, Icon::ChevronRight, "Jump to time\u{2026}", "Ctrl+G").clicked() {
        *open_jump = true;
        ui.close();
    }
    let ab_label = if ab.is_active() {
        "Clear A-B loop"
    } else if ab.a.is_some() {
        "Set loop end (B)"
    } else {
        "Set loop start (A)"
    };
    if menu_row(ui, Icon::Repeat, ab_label, "B").clicked() {
        *cycle_ab_loop = true;
    }
    if menu_row(ui, Icon::Info, "Media information", "I").clicked() {
        *show_info = true;
        ui.close();
    }
    if menu_row(ui, Icon::More, "Preferences\u{2026}", "Ctrl+P").clicked() {
        *show_preferences = true;
        ui.close();
    }
}

/// Compact pill used for option groups inside popups.
pub fn chip(ui: &mut Ui, selected: bool, text: &str) -> Response {
    let galley = ui.painter().layout_no_wrap(text.to_owned(), Theme::font(12.5), egui::Color32::PLACEHOLDER);
    let size = galley.size() + Vec2::new(18.0, 8.0);
    let (rect, resp) = ui.allocate_exact_size(size, Sense::click());
    let fill = if selected {
        Theme::TEXT
    } else if resp.hovered() {
        Theme::white(38)
    } else {
        Theme::white(16)
    };
    ui.painter().rect_filled(rect, Theme::radius_sm(), fill);
    let text_color = if selected { Theme::BG } else if resp.hovered() { Theme::TEXT } else { Theme::TEXT_MUTED };
    ui.painter().galley(rect.center() - galley.size() / 2.0, galley, text_color);
    resp
}

/// Full-width selectable row with a check mark when selected.
pub fn option_row(ui: &mut Ui, selected: bool, text: &str) -> Response {
    let (rect, resp) = ui.allocate_exact_size(Vec2::new(ui.available_width(), 30.0), Sense::click());
    if resp.hovered() {
        ui.painter().rect_filled(rect, Theme::radius_sm(), Theme::white(16));
    }
    let color = if selected || resp.hovered() { Theme::TEXT } else { Theme::TEXT_MUTED };
    let mut job = LayoutJob::simple_singleline(text.to_owned(), Theme::font(13.5), color);
    job.wrap = TextWrapping::truncate_at_width(rect.width() - 34.0);
    let galley = ui.painter().layout_job(job);
    ui.painter().galley(rect.left_center() + Vec2::new(8.0, -galley.size().y / 2.0), galley, color);
    if selected {
        let c = rect.right_center() - Vec2::new(14.0, 0.0);
        Icon::Check.paint(ui.painter(), Rect::from_center_size(c, Vec2::splat(14.0)), Theme::ACCENT);
    }
    resp
}

/// Menu row with a leading icon and an optional right-aligned shortcut.
pub fn menu_row(ui: &mut Ui, icon: Icon, text: &str, shortcut: &str) -> Response {
    let (rect, resp) = ui.allocate_exact_size(Vec2::new(ui.available_width(), 32.0), Sense::click());
    if resp.hovered() {
        ui.painter().rect_filled(rect, Theme::radius_sm(), Theme::white(16));
    }
    let color = if resp.hovered() { Theme::TEXT } else { Theme::TEXT_MUTED };
    let r = Rect::from_center_size(rect.left_center() + Vec2::new(16.0, 0.0), Vec2::splat(16.0));
    icon.paint(ui.painter(), r, color);
    ui.painter().text(rect.left_center() + Vec2::new(34.0, 0.0), Align2::LEFT_CENTER, text, Theme::font(13.5), color);
    if !shortcut.is_empty() {
        ui.painter().text(rect.right_center() - Vec2::new(8.0, 0.0), Align2::RIGHT_CENTER, shortcut, Theme::font(12.0), Theme::TEXT_FAINT);
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
