//! Shared building blocks for the floating dialogs: frame, header, toggle switch and buttons.

use super::icons::{icon_button, Icon};
use super::theme::Theme;
use eframe::egui::{self, Align2, Key, Pos2, Rect, Response, RichText, Sense, Ui, Vec2};

/// Shows a centered dialog with a title and close button. The content closure receives a
/// `close` flag it can set to dismiss the dialog.
pub fn show_dialog(
    ui: &Ui,
    title: &str,
    open: &mut bool,
    width: f32,
    add_contents: impl FnOnce(&mut Ui, &mut bool),
) {
    if !*open {
        return;
    }
    let ctx = ui.ctx();
    let mut close = false;

    egui::Window::new(title)
        .id(egui::Id::unique(("dialog", title)))
        .title_bar(false)
        .resizable(false)
        .collapsible(false)
        .anchor(Align2::CENTER_CENTER, Vec2::ZERO)
        .min_width(width)
        .max_width(width)
        .show(ctx, |ui| {
            ui.set_width(width);
            ui.horizontal(|ui| {
                ui.label(RichText::new(title).font(Theme::font_semibold(16.0)).color(Theme::TEXT));
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    if icon_button(ui, Icon::Close, 28.0).clicked() {
                        close = true;
                    }
                });
            });
            ui.add_space(12.0);
            add_contents(ui, &mut close);
        });

    if close || ctx.input(|i| i.key_pressed(Key::Escape)) {
        *open = false;
    }
}

/// A fixed-height row whose contents are laid out from the right edge (dialog footers).
pub fn footer_row(ui: &mut Ui, add_contents: impl FnOnce(&mut Ui)) {
    let size = Vec2::new(ui.available_width(), 32.0);
    ui.allocate_ui_with_layout(size, egui::Layout::right_to_left(egui::Align::Center), add_contents);
}

/// iOS-style toggle switch.
pub fn toggle(ui: &mut Ui, on: &mut bool) -> Response {
    let size = Vec2::new(36.0, 20.0);
    let (rect, mut resp) = ui.allocate_exact_size(size, Sense::click());
    if resp.clicked() {
        *on = !*on;
        resp.mark_changed();
    }
    let t = ui.ctx().animate_bool_with_time(resp.id, *on, 0.15);
    let painter = ui.painter();
    let track = super::icons::lerp_color(Theme::white(40), Theme::ACCENT, t);
    painter.rect_filled(rect, Theme::radius_full(), track);
    let r = rect.height() / 2.0 - 3.0;
    let x = egui::lerp((rect.min.x + r + 3.0)..=(rect.max.x - r - 3.0), t);
    painter.circle_filled(Pos2::new(x, rect.center().y), r, Theme::TEXT);
    resp
}

pub fn primary_button(text: &str) -> egui::Button<'static> {
    egui::Button::new(RichText::new(text.to_owned()).font(Theme::font_semibold(13.5)).color(Theme::BG))
        .fill(Theme::TEXT)
        .corner_radius(Theme::radius_sm())
        .min_size(Vec2::new(96.0, 32.0))
}

pub fn secondary_button(text: &str) -> egui::Button<'static> {
    egui::Button::new(RichText::new(text.to_owned()).color(Theme::TEXT))
        .fill(Theme::white(12))
        .corner_radius(Theme::radius_sm())
        .min_size(Vec2::new(84.0, 32.0))
}

/// A row with a muted label on the left and the control on the right.
pub fn setting_row(ui: &mut Ui, label: &str, add_control: impl FnOnce(&mut Ui)) {
    ui.horizontal(|ui| {
        ui.set_min_height(32.0);
        ui.label(RichText::new(label).color(Theme::TEXT_MUTED));
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), add_control);
    });
}

/// Two-option segmented control.
pub fn segmented<T: PartialEq + Copy>(ui: &mut Ui, value: &mut T, options: &[(T, &str)]) {
    let frame = egui::Frame::NONE
        .fill(Theme::white(8))
        .corner_radius(Theme::radius_md())
        .inner_margin(egui::Margin::same(3));
    frame.show(ui, |ui| {
        ui.horizontal(|ui| {
            ui.spacing_mut().item_spacing.x = 2.0;
            for (opt, label) in options {
                let selected = *value == *opt;
                let galley = ui.painter().layout_no_wrap(
                    (*label).to_owned(),
                    Theme::font(13.0),
                    if selected { Theme::TEXT } else { Theme::TEXT_MUTED },
                );
                let size = galley.size() + Vec2::new(28.0, 12.0);
                let (rect, resp) = ui.allocate_exact_size(size, Sense::click());
                if selected {
                    ui.painter().rect_filled(rect, Theme::radius_sm(), Theme::white(22));
                } else if resp.hovered() {
                    ui.painter().rect_filled(rect, Theme::radius_sm(), Theme::white(8));
                }
                ui.painter().galley(rect.center() - galley.size() / 2.0, galley, Theme::TEXT);
                if resp.clicked() {
                    *value = *opt;
                }
            }
        });
    });
}

/// Vertical gain slider used by the equalizer (centered at 0 dB).
pub fn band_slider(ui: &mut Ui, value: &mut f32, min: f32, max: f32, height: f32) -> Response {
    let (rect, mut resp) = ui.allocate_exact_size(Vec2::new(28.0, height), Sense::click_and_drag());
    let track = Rect::from_center_size(rect.center(), Vec2::new(4.0, height - 12.0));
    let to_y = |v: f32| egui::remap_clamp(v, min..=max, track.max.y..=track.min.y);

    if let Some(p) = resp.interact_pointer_pos() {
        if resp.dragged() || resp.clicked() {
            let v = egui::remap_clamp(p.y, track.max.y..=track.min.y, min..=max);
            let v = (v * 2.0).round() / 2.0;
            if v != *value {
                *value = v;
                resp.mark_changed();
            }
        }
    }
    if resp.double_clicked() {
        *value = 0.0;
        resp.mark_changed();
    }

    let painter = ui.painter();
    painter.rect_filled(track, Theme::radius_full(), Theme::white(30));
    let zero = to_y(0.0);
    let y = to_y(*value);
    let fill = Rect::from_min_max(Pos2::new(track.min.x, y.min(zero)), Pos2::new(track.max.x, y.max(zero)));
    painter.rect_filled(fill, Theme::radius_full(), Theme::ACCENT);
    painter.line_segment(
        [Pos2::new(track.min.x - 4.0, zero), Pos2::new(track.max.x + 4.0, zero)],
        egui::Stroke::new(1.0, Theme::white(60)),
    );
    let hot = ui.ctx().animate_bool_with_time(resp.id, resp.hovered() || resp.dragged(), 0.12);
    painter.circle_filled(Pos2::new(track.center().x, y), 6.0 + hot * 1.5, Theme::TEXT);
    resp
}
