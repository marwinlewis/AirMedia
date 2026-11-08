//! The slim now-playing bar shown at the bottom of the home screen, so music keeps going and a
//! video stays one click away while browsing.

use super::artwork::{paint_cover, ArtworkCache};
use super::controls::{format_time, volume_slider};
use super::icons::{icon_button, primary_circle_button, Icon};
use super::theme::Theme;
use crate::library::MediaKind;
use crate::media::MediaPlayer;
use eframe::egui::{self, text::LayoutJob, text::TextWrapping, Color32, Id, Pos2, Rect, Sense, TextureHandle, Ui, UiBuilder, Vec2};
use std::path::Path;

pub const MINI_PLAYER_HEIGHT: f32 = 72.0;

#[derive(Default)]
pub struct MiniPlayerOutput {
    pub expand: bool,
    pub stop: bool,
    pub next: bool,
    pub prev: bool,
}

pub struct MiniPlayerCx<'a> {
    pub path: &'a Path,
    pub title: &'a str,
    pub subtitle: &'a str,
    pub live_frame: Option<&'a TextureHandle>,
    pub art: &'a mut ArtworkCache,
}

pub fn show(ui: &mut Ui, rect: Rect, player: &mut MediaPlayer, cx: MiniPlayerCx<'_>) -> MiniPlayerOutput {
    let mut out = MiniPlayerOutput::default();
    let painter = ui.painter_at(rect);
    painter.rect_filled(rect, egui::CornerRadius::ZERO, Theme::SURFACE);

    let time = player.current_time_secs();
    let duration = player.duration_secs;

    // Progress along the top edge; click anywhere on it to seek.
    let seek = Rect::from_min_size(rect.min - Vec2::new(0.0, 3.0), Vec2::new(rect.width(), 9.0));
    let seek_resp = ui.interact(seek, Id::unique("mini_seek"), Sense::click()).on_hover_cursor(egui::CursorIcon::PointingHand);
    let rail = Rect::from_min_size(rect.min, Vec2::new(rect.width(), if seek_resp.hovered() { 4.0 } else { 3.0 }));
    painter.rect_filled(rail, egui::CornerRadius::ZERO, Theme::RAIL_BG);
    if duration > 0.0 {
        let frac = (time / duration).clamp(0.0, 1.0) as f32;
        let fill = Rect::from_min_size(rail.min, Vec2::new(rail.width() * frac, rail.height()));
        painter.rect_filled(fill, egui::CornerRadius::ZERO, Theme::ACCENT);
        if seek_resp.clicked() {
            if let Some(p) = seek_resp.interact_pointer_pos() {
                let f = ((p.x - rect.min.x) / rect.width()).clamp(0.0, 1.0) as f64;
                player.seek(f * duration);
            }
        }
    }

    let wide = rect.width() >= 760.0;
    let pad = 12.0;
    let thumb_h = rect.height() - 20.0;
    let thumb_w = thumb_h * 16.0 / 9.0;
    let thumb = Rect::from_min_size(rect.min + Vec2::new(pad, 11.0), Vec2::new(thumb_w, thumb_h));
    match cx.live_frame {
        Some(tex) => paint_cover(&painter, thumb, Theme::radius_sm(), tex, Color32::WHITE),
        None => cx.art.paint(&painter, thumb, Theme::radius_sm(), cx.path, cx.title, MediaKind::of(cx.path)),
    }

    // Right side: tools, then transport, whatever's left goes to the title.
    let tools_w = if wide { 300.0 } else { 40.0 };
    let tools = Rect::from_min_max(Pos2::new(rect.max.x - tools_w - pad, rect.min.y + 4.0), Pos2::new(rect.max.x - pad, rect.max.y));
    let mut tools_ui = ui.new_child(UiBuilder::new().id_salt("mini_tools").max_rect(tools).layout(egui::Layout::right_to_left(egui::Align::Center)));
    tools_ui.spacing_mut().item_spacing = Vec2::new(4.0, 0.0);
    if wide {
        if icon_button(&mut tools_ui, Icon::Close, 34.0).on_hover_text("Stop (S)").clicked() {
            out.stop = true;
        }
        if icon_button(&mut tools_ui, Icon::Expand, 34.0).on_hover_text("Open player").clicked() {
            out.expand = true;
        }
        let mut vol = if player.is_muted() { 0.0 } else { player.get_volume() };
        if volume_slider(&mut tools_ui, &mut vol) {
            if player.is_muted() {
                player.toggle_mute();
            }
            player.set_volume(vol);
        }
        let icon = if vol <= 0.001 {
            Icon::VolumeMute
        } else if vol < 0.5 {
            Icon::VolumeLow
        } else {
            Icon::VolumeHigh
        };
        if icon_button(&mut tools_ui, icon, 34.0).on_hover_text("Mute (M)").clicked() {
            player.toggle_mute();
        }
    } else if icon_button(&mut tools_ui, Icon::Close, 34.0).on_hover_text("Stop (S)").clicked() {
        out.stop = true;
    }

    let transport_w = if wide { 136.0 } else { 44.0 };
    let transport = Rect::from_center_size(
        if wide {
            Pos2::new(rect.center().x, rect.center().y + 2.0)
        } else {
            Pos2::new(tools.min.x - transport_w / 2.0, rect.center().y + 2.0)
        },
        Vec2::new(transport_w, 48.0),
    );
    let mut transport_ui = ui.new_child(UiBuilder::new().id_salt("mini_transport").max_rect(transport).layout(egui::Layout::left_to_right(egui::Align::Center)));
    transport_ui.spacing_mut().item_spacing = Vec2::new(4.0, 0.0);
    if wide && icon_button(&mut transport_ui, Icon::Prev, 40.0).on_hover_text("Previous (P)").clicked() {
        out.prev = true;
    }
    let icon = if player.is_playing { Icon::Pause } else { Icon::Play };
    if primary_circle_button(&mut transport_ui, icon, 44.0, Theme::TEXT, Theme::BG)
        .on_hover_text(if player.is_playing { "Pause (Space)" } else { "Play (Space)" })
        .clicked()
    {
        player.toggle_play();
    }
    if wide && icon_button(&mut transport_ui, Icon::Next, 40.0).on_hover_text("Next (N)").clicked() {
        out.next = true;
    }

    // Title block: clicking it, or the thumbnail, opens the player.
    let text_x = thumb.max.x + 12.0;
    let text_end = (transport.min.x - 12.0).max(text_x + 40.0);
    let info = Rect::from_min_max(Pos2::new(thumb.min.x, rect.min.y + 6.0), Pos2::new(text_end, rect.max.y));
    let info_resp = ui.interact(info, Id::unique("mini_info"), Sense::click()).on_hover_cursor(egui::CursorIcon::PointingHand).on_hover_text("Open player");
    if info_resp.clicked() {
        out.expand = true;
    }

    let max_w = text_end - text_x;
    let title_color = if info_resp.hovered() { Theme::ACCENT_SOFT } else { Theme::TEXT };
    let line = |text: &str, font, color| {
        let mut job = LayoutJob::simple_singleline(text.to_owned(), font, color);
        job.wrap = TextWrapping::truncate_at_width(max_w);
        painter.layout_job(job)
    };
    let cy = rect.center().y + 2.0;
    painter.galley(Pos2::new(text_x, cy - 20.0), line(cx.title, Theme::font_semibold(14.5), title_color), title_color);
    let status = if duration > 0.0 {
        format!("{} / {}", format_time(time), format_time(duration))
    } else {
        format_time(time)
    };
    let sub_text = if cx.subtitle.is_empty() { status } else { format!("{status}  \u{00b7}  {}", cx.subtitle) };
    painter.galley(Pos2::new(text_x, cy + 2.0), line(&sub_text, Theme::font(12.5), Theme::TEXT_MUTED), Theme::TEXT_MUTED);

    out
}
