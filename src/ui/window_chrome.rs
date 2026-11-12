//! Custom borderless window chrome: floating glass Min / Max / Close capsule, titlebar drag &
//! double-click maximize, edge/corner resizing, and a subtle hairline window border.

use super::icons::lerp_color;
use super::theme::Theme;
use eframe::egui::{
    self, Color32, CornerRadius, CursorIcon, Id, LayerId, Order, Painter, PointerButton, Popup,
    Pos2, Rect, ResizeDirection, Sense, Stroke, StrokeKind, Ui, UiBuilder, Vec2, ViewportCommand,
};

/// Custom window chrome is active on desktop platforms; Android uses its native activity.
pub const CUSTOM_CHROME: bool = !cfg!(target_os = "android");

const RESIZE_EDGE: f32 = 5.0;

#[derive(Clone, Copy)]
struct ChromeMetrics {
    capsule_w: f32,
    capsule_h: f32,
    right_margin: f32,
    gap_to_tools: f32,
    btn_w: f32,
    btn_h: f32,
}

fn metrics(bar_width: f32) -> ChromeMetrics {
    if bar_width < 600.0 {
        ChromeMetrics {
            capsule_w: 92.0,
            capsule_h: 30.0,
            right_margin: 10.0,
            gap_to_tools: 8.0,
            btn_w: 28.0,
            btn_h: 24.0,
        }
    } else {
        ChromeMetrics {
            capsule_w: 104.0,
            capsule_h: 34.0,
            right_margin: 16.0,
            gap_to_tools: 12.0,
            btn_w: 32.0,
            btn_h: 28.0,
        }
    }
}

/// Horizontal space reserved at the right of the top bar so navigation/menu tools shift left
/// cleanly to make room for the floating window controls capsule.
pub fn right_reserve(bar_width: f32, is_fullscreen: bool) -> f32 {
    if !CUSTOM_CHROME || is_fullscreen {
        return 0.0;
    }
    let m = metrics(bar_width);
    m.right_margin + m.capsule_w + m.gap_to_tools
}

/// Makes the empty background of a top bar swallow pointer clicks and, on desktop outside
/// fullscreen, draggable to move the window and double-clickable to toggle maximize/restore.
/// Call this *before* adding interactive buttons inside `rect`.
pub fn handle_titlebar_drag(ui: &mut Ui, rect: Rect, id: Id, is_fullscreen: bool) {
    let sense = if CUSTOM_CHROME && !is_fullscreen {
        Sense::click_and_drag()
    } else {
        Sense::click()
    };
    let resp = ui.interact(rect, id, sense);
    if !CUSTOM_CHROME || is_fullscreen || resize_hit(ui.ctx(), is_fullscreen).is_some() {
        return;
    }
    if resp.double_clicked() {
        let maximized = ui.ctx().input(|i| i.viewport().maximized.unwrap_or(false));
        ui.ctx().send_viewport_cmd(ViewportCommand::Maximized(!maximized));
    } else if resp.drag_started_by(PointerButton::Primary) {
        ui.ctx().send_viewport_cmd(ViewportCommand::StartDrag);
    }
}

/// Paints the floating glass `[Minimize | Maximize | Close]` capsule at the right end of
/// `bar_rect`, vertically centered. Hidden on Android and in fullscreen.
pub fn show_window_controls(ui: &mut Ui, bar_rect: Rect, is_fullscreen: bool) {
    if !CUSTOM_CHROME || is_fullscreen {
        return;
    }
    let m = metrics(bar_rect.width());
    let cy = bar_rect.center().y;
    let capsule_rect = Rect::from_center_size(
        Pos2::new(bar_rect.max.x - m.right_margin - m.capsule_w * 0.5, cy),
        Vec2::new(m.capsule_w, m.capsule_h),
    );

    let painter = ui.painter();
    painter.rect_filled(capsule_rect, Theme::radius_full(), Color32::from_black_alpha(180));
    painter.rect_stroke(
        capsule_rect,
        Theme::radius_full(),
        Stroke::new(1.0, Theme::white(28)),
        StrokeKind::Inside,
    );

    let inner = capsule_rect.shrink2(Vec2::new(3.0, (m.capsule_h - m.btn_h) * 0.5));
    let mut row = ui.new_child(
        UiBuilder::new()
            .id_salt("window_controls_capsule")
            .max_rect(inner)
            .layout(egui::Layout::left_to_right(egui::Align::Center)),
    );
    row.spacing_mut().item_spacing = Vec2::new(1.0, 0.0);

    let ctx = ui.ctx().clone();
    let maximized = ctx.input(|i| i.viewport().maximized.unwrap_or(false));

    if caption_button(&mut row, CaptionButton::Minimize, Vec2::new(m.btn_w, m.btn_h))
        .on_hover_text("Minimize")
        .clicked()
    {
        ctx.send_viewport_cmd(ViewportCommand::Minimized(true));
    }

    let max_kind = if maximized {
        CaptionButton::Restore
    } else {
        CaptionButton::Maximize
    };
    let max_tip = if maximized { "Restore" } else { "Maximize" };
    if caption_button(&mut row, max_kind, Vec2::new(m.btn_w, m.btn_h))
        .on_hover_text(max_tip)
        .clicked()
    {
        ctx.send_viewport_cmd(ViewportCommand::Maximized(!maximized));
    }

    if caption_button(&mut row, CaptionButton::Close, Vec2::new(m.btn_w, m.btn_h))
        .on_hover_text("Close")
        .clicked()
    {
        ctx.send_viewport_cmd(ViewportCommand::Close);
    }
}

fn resize_hit(ctx: &egui::Context, is_fullscreen: bool) -> Option<(ResizeDirection, CursorIcon)> {
    if !CUSTOM_CHROME || is_fullscreen {
        return None;
    }
    if ctx.input(|i| i.viewport().maximized.unwrap_or(false)) {
        return None;
    }
    let pos = ctx.input(|i| i.pointer.hover_pos())?;
    let rect = ctx.viewport_rect();
    let left = pos.x <= rect.min.x + RESIZE_EDGE;
    let right = pos.x >= rect.max.x - RESIZE_EDGE;
    let top = pos.y <= rect.min.y + RESIZE_EDGE;
    let bottom = pos.y >= rect.max.y - RESIZE_EDGE;

    match (left, right, top, bottom) {
        (true, _, true, _) => Some((ResizeDirection::NorthWest, CursorIcon::ResizeNwSe)),
        (_, true, true, _) => Some((ResizeDirection::NorthEast, CursorIcon::ResizeNeSw)),
        (true, _, _, true) => Some((ResizeDirection::SouthWest, CursorIcon::ResizeNeSw)),
        (_, true, _, true) => Some((ResizeDirection::SouthEast, CursorIcon::ResizeNwSe)),
        (true, _, _, _) => Some((ResizeDirection::West, CursorIcon::ResizeHorizontal)),
        (_, true, _, _) => Some((ResizeDirection::East, CursorIcon::ResizeHorizontal)),
        (_, _, true, _) => Some((ResizeDirection::North, CursorIcon::ResizeVertical)),
        (_, _, _, true) => Some((ResizeDirection::South, CursorIcon::ResizeVertical)),
        _ => None,
    }
}

/// Handles edge/corner resize cursors and drag initiation when unmaximized and not fullscreen.
/// Call before laying out panels so edge presses win over background drag areas.
pub fn handle_edge_resize(ctx: &egui::Context, is_fullscreen: bool) {
    if ctx.egui_is_using_pointer() || Popup::is_any_open(ctx) {
        return;
    }
    if let Some((dir, cursor)) = resize_hit(ctx, is_fullscreen) {
        ctx.set_cursor_icon(cursor);
        if ctx.input(|i| i.pointer.primary_pressed()) {
            ctx.send_viewport_cmd(ViewportCommand::BeginResize(dir));
        }
    }
}

/// Draws a subtle 1px hairline window border when unmaximized and not fullscreen.
pub fn paint_window_border(ctx: &egui::Context, is_fullscreen: bool) {
    if !CUSTOM_CHROME || is_fullscreen {
        return;
    }
    if ctx.input(|i| i.viewport().maximized.unwrap_or(false)) {
        return;
    }
    let rect = ctx.viewport_rect();
    let painter = ctx.layer_painter(LayerId::new(Order::Foreground, Id::unique("window_chrome_border")));
    painter.rect_stroke(rect, CornerRadius::ZERO, Stroke::new(1.0, Theme::BORDER), StrokeKind::Inside);
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum CaptionButton {
    Minimize,
    Maximize,
    Restore,
    Close,
}

fn caption_button(ui: &mut Ui, kind: CaptionButton, size: Vec2) -> egui::Response {
    let (rect, resp) = ui.allocate_exact_size(size, Sense::click());
    if ui.is_rect_visible(rect) {
        let hover = ui.ctx().animate_bool_with_time(resp.id, resp.hovered(), 0.12);
        let painter = ui.painter();

        if hover > 0.0 {
            let fill = if kind == CaptionButton::Close {
                Color32::from_rgb(220, 56, 56).gamma_multiply(hover)
            } else {
                Theme::white((28.0 * hover) as u8)
            };
            painter.rect_filled(rect, Theme::radius_full(), fill);
        }

        let fg = lerp_color(Theme::TEXT_MUTED, Theme::TEXT, hover);
        let scale = if resp.is_pointer_button_down_on() { 0.9 } else { 1.0 };
        paint_caption_glyph(painter, rect.center(), kind, fg, scale);
    }
    resp
}

fn paint_caption_glyph(painter: &Painter, c: Pos2, kind: CaptionButton, color: Color32, scale: f32) {
    let stroke = Stroke::new(1.5, color);
    match kind {
        CaptionButton::Minimize => {
            let half = 4.5 * scale;
            painter.line_segment([Pos2::new(c.x - half, c.y + 0.5), Pos2::new(c.x + half, c.y + 0.5)], stroke);
        }
        CaptionButton::Maximize => {
            let box_rect = Rect::from_center_size(c, Vec2::splat(8.5 * scale));
            painter.rect_stroke(box_rect, CornerRadius::same(1), stroke, StrokeKind::Middle);
        }
        CaptionButton::Restore => {
            let s = 6.5 * scale;
            let off = 2.0 * scale;
            let front = Rect::from_center_size(Pos2::new(c.x - off * 0.5, c.y + off * 0.5), Vec2::splat(s));
            painter.rect_stroke(front, CornerRadius::same(1), stroke, StrokeKind::Middle);
            // Back square's exposed top-right L-corner.
            let back = front.translate(Vec2::new(off, -off));
            painter.line_segment([Pos2::new(back.min.x + 1.5, back.min.y), back.right_top()], stroke);
            painter.line_segment([back.right_top(), Pos2::new(back.max.x, back.max.y - 1.5)], stroke);
        }
        CaptionButton::Close => {
            let half = 4.2 * scale;
            let close_stroke = Stroke::new(1.6, color);
            painter.line_segment([Pos2::new(c.x - half, c.y - half), Pos2::new(c.x + half, c.y + half)], close_stroke);
            painter.line_segment([Pos2::new(c.x + half, c.y - half), Pos2::new(c.x - half, c.y + half)], close_stroke);
        }
    }
}
