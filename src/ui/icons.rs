//! Small vector icon set painted directly with egui shapes, so the UI does not depend on
//! emoji / symbol font coverage and stays crisp at any DPI.

use super::theme::Theme;
use eframe::egui::{
    self, epaint::PathShape, epaint::PathStroke, Color32, Painter, Pos2, Rect, Response, Sense,
    Stroke, Ui, Vec2,
};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Icon {
    Play,
    Pause,
    Prev,
    Next,
    StepForward,
    StepBackward,
    VolumeHigh,
    VolumeLow,
    VolumeMute,
    Fullscreen,
    ExitFullscreen,
    Playlist,
    Subtitles,
    Settings,
    Camera,
    Menu,
    Close,
    Plus,
    Shuffle,
    Repeat,
    Search,
    Trash,
    More,
    ArrowLeft,
    ChevronLeft,
    ChevronRight,
    Check,
    Info,
    Back10,
    Forward10,
    Speed,
    Folder,
    Film,
    Music,
    Globe,
    Expand,
}

impl Icon {
    pub fn paint(self, painter: &Painter, rect: Rect, color: Color32) {
        let c = rect.center();
        let s = rect.width().min(rect.height());
        let u = s / 24.0; // design grid: 24x24
        let p = |x: f32, y: f32| Pos2::new(c.x + (x - 12.0) * u, c.y + (y - 12.0) * u);
        let stroke = Stroke::new(1.8 * u, color);
        let line = |a: Pos2, b: Pos2| painter.line_segment([a, b], stroke);
        let poly = |pts: Vec<Pos2>| {
            painter.add(PathShape::convex_polygon(pts, color, Stroke::NONE));
        };
        let open_path = |pts: Vec<Pos2>| {
            painter.add(PathShape::line(pts, PathStroke::new(1.8 * u, color)));
        };

        match self {
            Icon::Play => poly(vec![p(7.5, 4.5), p(19.5, 12.0), p(7.5, 19.5)]),
            Icon::Pause => {
                let r = egui::CornerRadius::same((1.2 * u) as u8);
                painter.rect_filled(Rect::from_min_max(p(6.5, 5.0), p(10.0, 19.0)), r, color);
                painter.rect_filled(Rect::from_min_max(p(14.0, 5.0), p(17.5, 19.0)), r, color);
            }
            Icon::Prev => {
                poly(vec![p(18.0, 5.5), p(18.0, 18.5), p(8.5, 12.0)]);
                painter.rect_filled(
                    Rect::from_min_max(p(5.5, 5.5), p(7.5, 18.5)),
                    egui::CornerRadius::same((0.8 * u) as u8),
                    color,
                );
            }
            Icon::Next => {
                poly(vec![p(6.0, 5.5), p(6.0, 18.5), p(15.5, 12.0)]);
                painter.rect_filled(
                    Rect::from_min_max(p(16.5, 5.5), p(18.5, 18.5)),
                    egui::CornerRadius::same((0.8 * u) as u8),
                    color,
                );
            }
            Icon::StepForward => {
                painter.rect_stroke(
                    Rect::from_min_max(p(3.5, 5.5), p(20.5, 18.5)),
                    egui::CornerRadius::same((2.0 * u) as u8),
                    Stroke::new(1.4 * u, color),
                    egui::StrokeKind::Middle,
                );
                poly(vec![p(9.5, 8.5), p(9.5, 15.5), p(15.5, 12.0)]);
            }
            Icon::StepBackward => {
                painter.rect_stroke(
                    Rect::from_min_max(p(3.5, 5.5), p(20.5, 18.5)),
                    egui::CornerRadius::same((2.0 * u) as u8),
                    Stroke::new(1.4 * u, color),
                    egui::StrokeKind::Middle,
                );
                poly(vec![p(14.5, 8.5), p(14.5, 15.5), p(8.5, 12.0)]);
            }
            Icon::VolumeHigh | Icon::VolumeLow | Icon::VolumeMute => {
                poly(vec![p(3.5, 9.0), p(7.5, 9.0), p(12.5, 4.5), p(12.5, 19.5), p(7.5, 15.0), p(3.5, 15.0)]);
                match self {
                    Icon::VolumeMute => {
                        line(p(16.0, 9.0), p(22.0, 15.0));
                        line(p(22.0, 9.0), p(16.0, 15.0));
                    }
                    _ => {
                        arc(painter, p(12.5, 12.0), 4.0 * u, -0.9, 0.9, stroke);
                        if self == Icon::VolumeHigh {
                            arc(painter, p(12.5, 12.0), 8.0 * u, -0.95, 0.95, stroke);
                        }
                    }
                }
            }
            Icon::Fullscreen => {
                open_path(vec![p(4.0, 9.0), p(4.0, 4.0), p(9.0, 4.0)]);
                open_path(vec![p(15.0, 4.0), p(20.0, 4.0), p(20.0, 9.0)]);
                open_path(vec![p(20.0, 15.0), p(20.0, 20.0), p(15.0, 20.0)]);
                open_path(vec![p(9.0, 20.0), p(4.0, 20.0), p(4.0, 15.0)]);
            }
            Icon::ExitFullscreen => {
                open_path(vec![p(4.0, 9.0), p(9.0, 9.0), p(9.0, 4.0)]);
                open_path(vec![p(15.0, 4.0), p(15.0, 9.0), p(20.0, 9.0)]);
                open_path(vec![p(20.0, 15.0), p(15.0, 15.0), p(15.0, 20.0)]);
                open_path(vec![p(9.0, 20.0), p(9.0, 15.0), p(4.0, 15.0)]);
            }
            Icon::Playlist => {
                line(p(4.0, 7.0), p(20.0, 7.0));
                line(p(4.0, 12.0), p(16.0, 12.0));
                line(p(4.0, 17.0), p(12.0, 17.0));
            }
            Icon::Subtitles => {
                painter.rect_stroke(
                    Rect::from_min_max(p(3.0, 5.5), p(21.0, 18.5)),
                    egui::CornerRadius::same((2.5 * u) as u8),
                    stroke,
                    egui::StrokeKind::Middle,
                );
                line(p(7.0, 12.0), p(11.0, 12.0));
                line(p(13.5, 12.0), p(17.0, 12.0));
                line(p(7.0, 15.0), p(15.0, 15.0));
            }
            Icon::Settings => {
                for (y, knob) in [(6.0, 15.0), (12.0, 8.0), (18.0, 13.0)] {
                    line(p(4.0, y), p(20.0, y));
                    painter.circle_filled(p(knob, y), 2.6 * u, color);
                }
            }
            Icon::Camera => {
                painter.rect_stroke(
                    Rect::from_min_max(p(3.0, 7.5), p(21.0, 19.5)),
                    egui::CornerRadius::same((2.0 * u) as u8),
                    stroke,
                    egui::StrokeKind::Middle,
                );
                open_path(vec![p(8.0, 7.5), p(9.5, 5.0), p(14.5, 5.0), p(16.0, 7.5)]);
                painter.circle_stroke(p(12.0, 13.5), 3.6 * u, stroke);
                painter.circle_filled(p(18.0, 10.0), 0.9 * u, color);
            }
            Icon::Menu => {
                line(p(4.0, 7.0), p(20.0, 7.0));
                line(p(4.0, 12.0), p(20.0, 12.0));
                line(p(4.0, 17.0), p(20.0, 17.0));
            }
            Icon::Close => {
                line(p(6.0, 6.0), p(18.0, 18.0));
                line(p(18.0, 6.0), p(6.0, 18.0));
            }
            Icon::Plus => {
                line(p(12.0, 5.0), p(12.0, 19.0));
                line(p(5.0, 12.0), p(19.0, 12.0));
            }
            Icon::Shuffle => {
                open_path(vec![p(3.5, 7.0), p(7.5, 7.0), p(15.5, 17.0), p(20.0, 17.0)]);
                open_path(vec![p(3.5, 17.0), p(7.5, 17.0), p(15.5, 7.0), p(20.0, 7.0)]);
                open_path(vec![p(17.5, 4.5), p(20.0, 7.0), p(17.5, 9.5)]);
                open_path(vec![p(17.5, 14.5), p(20.0, 17.0), p(17.5, 19.5)]);
            }
            Icon::Repeat => {
                open_path(vec![p(4.0, 11.0), p(4.0, 7.5), p(19.0, 7.5)]);
                open_path(vec![p(16.5, 5.0), p(19.0, 7.5), p(16.5, 10.0)]);
                open_path(vec![p(20.0, 13.0), p(20.0, 16.5), p(5.0, 16.5)]);
                open_path(vec![p(7.5, 14.0), p(5.0, 16.5), p(7.5, 19.0)]);
            }
            Icon::Search => {
                painter.circle_stroke(p(10.5, 10.5), 5.5 * u, stroke);
                line(p(14.5, 14.5), p(19.5, 19.5));
            }
            Icon::Trash => {
                line(p(4.0, 6.5), p(20.0, 6.5));
                open_path(vec![p(9.0, 6.5), p(9.5, 4.0), p(14.5, 4.0), p(15.0, 6.5)]);
                open_path(vec![p(6.0, 6.5), p(7.0, 20.0), p(17.0, 20.0), p(18.0, 6.5)]);
            }
            Icon::More => {
                let r = 1.6 * u;
                painter.circle_filled(p(6.0, 12.0), r, color);
                painter.circle_filled(p(12.0, 12.0), r, color);
                painter.circle_filled(p(18.0, 12.0), r, color);
            }
            Icon::ArrowLeft => {
                line(p(20.0, 12.0), p(4.5, 12.0));
                open_path(vec![p(11.0, 5.0), p(4.0, 12.0), p(11.0, 19.0)]);
            }
            Icon::ChevronLeft => open_path(vec![p(15.0, 4.5), p(7.5, 12.0), p(15.0, 19.5)]),
            Icon::ChevronRight => open_path(vec![p(9.0, 4.5), p(16.5, 12.0), p(9.0, 19.5)]),
            Icon::Check => open_path(vec![p(4.5, 12.5), p(10.0, 18.0), p(19.5, 6.5)]),
            Icon::Info => {
                painter.circle_stroke(p(12.0, 12.0), 9.0 * u, stroke);
                painter.circle_filled(p(12.0, 7.8), 1.2 * u, color);
                line(p(12.0, 11.0), p(12.0, 17.0));
            }
            Icon::Back10 | Icon::Forward10 => {
                let forward = self == Icon::Forward10;
                let center = p(12.0, 13.0);
                let top = -std::f32::consts::FRAC_PI_2;
                let sweep = 5.2;
                let (from, to) = if forward { (top, top - sweep) } else { (top, top + sweep) };
                arc(painter, center, 8.5 * u, from, to, stroke);
                let (tip, wing_x) = if forward { (13.0, 10.0) } else { (11.0, 14.0) };
                open_path(vec![p(wing_x, 1.5), p(tip, 4.5), p(wing_x, 7.5)]);
                painter.text(
                    center,
                    egui::Align2::CENTER_CENTER,
                    "10",
                    Theme::font_semibold(7.6 * u),
                    color,
                );
            }
            Icon::Speed => {
                arc(painter, p(12.0, 14.0), 9.0 * u, 2.65, 6.78, stroke);
                line(p(12.0, 14.0), p(16.5, 8.5));
                painter.circle_filled(p(12.0, 14.0), 1.8 * u, color);
            }
            Icon::Folder => {
                open_path(vec![
                    p(3.0, 18.5),
                    p(3.0, 5.5),
                    p(9.5, 5.5),
                    p(11.5, 8.0),
                    p(21.0, 8.0),
                    p(21.0, 18.5),
                    p(3.0, 18.5),
                ]);
            }
            Icon::Film => {
                painter.rect_stroke(
                    Rect::from_min_max(p(3.0, 4.5), p(21.0, 19.5)),
                    egui::CornerRadius::same((1.8 * u) as u8),
                    stroke,
                    egui::StrokeKind::Middle,
                );
                line(p(7.5, 4.5), p(7.5, 19.5));
                line(p(16.5, 4.5), p(16.5, 19.5));
                line(p(3.0, 12.0), p(21.0, 12.0));
            }
            Icon::Music => {
                open_path(vec![p(9.0, 17.5), p(9.0, 5.5), p(20.0, 3.5), p(20.0, 15.5)]);
                painter.circle_filled(p(6.5, 17.5), 2.8 * u, color);
                painter.circle_filled(p(17.5, 15.5), 2.8 * u, color);
            }
            Icon::Globe => {
                painter.circle_stroke(p(12.0, 12.0), 9.0 * u, stroke);
                line(p(3.0, 12.0), p(21.0, 12.0));
                let lens: Vec<Pos2> = (0..=24)
                    .map(|i| {
                        let t = i as f32 / 24.0 * std::f32::consts::TAU;
                        Pos2::new(c.x + t.cos() * 4.0 * u, c.y + t.sin() * 9.0 * u)
                    })
                    .collect();
                painter.add(PathShape::line(lens, PathStroke::new(1.8 * u, color)));
            }
            Icon::Expand => {
                open_path(vec![p(13.5, 4.0), p(20.0, 4.0), p(20.0, 10.5)]);
                open_path(vec![p(10.5, 20.0), p(4.0, 20.0), p(4.0, 13.5)]);
                line(p(20.0, 4.0), p(13.5, 10.5));
                line(p(4.0, 20.0), p(10.5, 13.5));
            }
        }
    }
}

fn arc(painter: &Painter, center: Pos2, radius: f32, from: f32, to: f32, stroke: Stroke) {
    let steps = 14;
    let pts: Vec<Pos2> = (0..=steps)
        .map(|i| {
            let t = from + (to - from) * (i as f32 / steps as f32);
            center + Vec2::new(t.cos(), t.sin()) * radius
        })
        .collect();
    painter.add(PathShape::line(pts, PathStroke::new(stroke.width, stroke.color)));
}

/// A borderless round icon button: invisible at rest, soft highlight on hover.
pub fn icon_button(ui: &mut Ui, icon: Icon, size: f32) -> Response {
    icon_button_ex(ui, icon, size, false)
}

/// Like [`icon_button`], but drawn in the accent color when `active`.
pub fn icon_button_ex(ui: &mut Ui, icon: Icon, size: f32, active: bool) -> Response {
    let (rect, resp) = ui.allocate_exact_size(Vec2::splat(size), Sense::click());
    if ui.is_rect_visible(rect) {
        let hover = ui.ctx().animate_bool_with_time(resp.id, resp.hovered(), 0.12);
        let painter = ui.painter();
        if hover > 0.0 {
            painter.circle_filled(rect.center(), size * 0.5, Theme::white((26.0 * hover) as u8));
        }
        let color = if active {
            Theme::ACCENT
        } else {
            lerp_color(Theme::TEXT_MUTED, Theme::TEXT, hover)
        };
        let pressed = if resp.is_pointer_button_down_on() { 0.92 } else { 1.0 };
        let icon_rect = Rect::from_center_size(rect.center(), Vec2::splat(size * 0.55 * pressed));
        icon.paint(painter, icon_rect, color);
    }
    resp
}

/// A primary prominent circle button, like the Play/Pause button in the design:
/// Filled with filled_color, icon drawn in icon_color.
pub fn primary_circle_button(ui: &mut Ui, icon: Icon, size: f32, fill_color: Color32, icon_color: Color32) -> Response {
    let (rect, resp) = ui.allocate_exact_size(Vec2::splat(size), Sense::click());
    if ui.is_rect_visible(rect) {
        let hover = ui.ctx().animate_bool_with_time(resp.id, resp.hovered(), 0.12);
        let painter = ui.painter();
        let pressed = if resp.is_pointer_button_down_on() { 0.94 } else { 1.0 };
        let radius = size * 0.5 * pressed;

        let bg = if hover > 0.0 {
            lerp_color(fill_color, Color32::WHITE, hover * 0.15)
        } else {
            fill_color
        };
        painter.circle_filled(rect.center(), radius, bg);

        // Subtle shadow / depth ring
        painter.circle_stroke(
            rect.center(),
            radius,
            Stroke::new(1.0, Theme::black(30)),
        );

        // Center the icon (adjusting play icon slightly for optical centering)
        let mut icon_center = rect.center();
        if icon == Icon::Play {
            icon_center.x += 1.8 * (size / 58.0);
        }
        let icon_rect = Rect::from_center_size(icon_center, Vec2::splat(size * 0.44 * pressed));
        icon.paint(painter, icon_rect, icon_color);
    }
    resp
}

/// Look of a [`pill_button`].
#[derive(Clone, Copy, PartialEq, Eq)]
pub enum PillStyle {
    /// Solid white: the one main action on a screen.
    Primary,
    /// Translucent grey: secondary actions.
    Secondary,
}

/// The hero-style call-to-action button: optional icon + label on a rounded rectangle.
pub fn pill_button(ui: &mut Ui, icon: Option<Icon>, label: &str, style: PillStyle, height: f32) -> Response {
    let font = Theme::font_semibold((height * 0.38).clamp(13.0, 16.5));
    let fg = if style == PillStyle::Primary { Theme::BG } else { Theme::TEXT };
    let galley = ui.painter().layout_no_wrap(label.to_owned(), font, fg);
    let icon_size = height * 0.5;
    let icon_w = if icon.is_some() { icon_size + height * 0.2 } else { 0.0 };
    let size = Vec2::new(galley.size().x + icon_w + height * 0.9, height);
    let (rect, resp) = ui.allocate_exact_size(size, Sense::click());
    if ui.is_rect_visible(rect) {
        let hover = ui.ctx().animate_bool_with_time(resp.id, resp.hovered(), 0.12);
        let fill = match style {
            PillStyle::Primary => lerp_color(Theme::TEXT, Color32::from_rgb(214, 214, 222), hover),
            PillStyle::Secondary => lerp_color(Color32::from_rgb(58, 58, 70), Color32::from_rgb(82, 82, 96), hover),
        };
        let painter = ui.painter();
        painter.rect_filled(rect, Theme::radius_md(), fill);
        if resp.has_focus() {
            painter.rect_stroke(rect.expand(2.0), Theme::radius_md(), Stroke::new(2.0, Theme::TEXT), egui::StrokeKind::Outside);
        }
        let mut x = rect.min.x + height * 0.45;
        if let Some(icon) = icon {
            let r = Rect::from_center_size(Pos2::new(x + icon_size / 2.0, rect.center().y), Vec2::splat(icon_size));
            icon.paint(painter, r, fg);
            x += icon_w;
        }
        painter.galley(Pos2::new(x, rect.center().y - galley.size().y / 2.0), galley, fg);
    }
    resp
}

pub fn lerp_color(a: Color32, b: Color32, t: f32) -> Color32 {
    let t = t.clamp(0.0, 1.0);
    let l = |x: u8, y: u8| (x as f32 + (y as f32 - x as f32) * t).round() as u8;
    Color32::from_rgba_premultiplied(l(a.r(), b.r()), l(a.g(), b.g()), l(a.b(), b.b()), l(a.a(), b.a()))
}
