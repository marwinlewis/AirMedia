use eframe::egui::{
    self, Color32, CornerRadius, FontData, FontDefinitions, FontFamily, FontId, Margin, Shadow,
    Stroke, StrokeKind, Style, TextStyle, Visuals,
};
use std::sync::Arc;

pub struct Theme;

impl Theme {
    // Surfaces matching the warm dark matte aesthetic in the design
    pub const BG: Color32 = Color32::from_rgb(22, 21, 20); // #161514
    pub const SURFACE: Color32 = Color32::from_rgb(28, 26, 25); // #1C1A19
    pub const CAPSULE_BG: Color32 = Color32::from_rgb(17, 16, 15); // #11100F

    // Accent: Warm cream / beige from the play button and progress bar
    pub const ACCENT: Color32 = Color32::from_rgb(237, 224, 208); // #EDE0D0
    pub const ACCENT_SOFT: Color32 = Color32::from_rgb(245, 239, 230); // #F5EFE6
    pub const ACCENT_DARK: Color32 = Color32::from_rgb(24, 22, 20); // #181614

    // Text
    pub const TEXT: Color32 = Color32::from_rgb(245, 239, 230); // #F5EFE6
    pub const TEXT_MUTED: Color32 = Color32::from_rgb(163, 155, 145); // #A39B91
    pub const TEXT_FAINT: Color32 = Color32::from_rgb(112, 106, 99); // #706A63

    // Hairlines & Tracks
    pub const BORDER: Color32 = Color32::from_rgb(38, 35, 33); // #262321
    pub const RAIL_BG: Color32 = Color32::from_rgb(46, 43, 40); // #2E2B28

    pub fn radius_sm() -> CornerRadius {
        CornerRadius::same(8)
    }

    pub fn radius_md() -> CornerRadius {
        CornerRadius::same(14)
    }

    pub fn radius_lg() -> CornerRadius {
        CornerRadius::same(20)
    }

    pub fn radius_xl() -> CornerRadius {
        CornerRadius::same(24)
    }

    pub fn radius_full() -> CornerRadius {
        CornerRadius::same(255)
    }

    pub fn stroke_kind() -> StrokeKind {
        StrokeKind::Inside
    }

    /// White with the given alpha (0..=255), premultiplied.
    pub fn white(alpha: u8) -> Color32 {
        Color32::from_white_alpha(alpha)
    }

    /// Black with the given alpha (0..=255), premultiplied.
    pub fn black(alpha: u8) -> Color32 {
        Color32::from_black_alpha(alpha)
    }

    pub fn font(size: f32) -> FontId {
        FontId::proportional(size)
    }

    pub fn font_semibold(size: f32) -> FontId {
        FontId::new(size, FontFamily::Name("semibold".into()))
    }

    pub fn font_bold(size: f32) -> FontId {
        FontId::new(size, FontFamily::Name("semibold".into()))
    }

    /// Fills `rect` with a top-to-bottom gradient (used for the overlay scrims).
    pub fn paint_vertical_gradient(painter: &egui::Painter, rect: egui::Rect, top: Color32, bottom: Color32) {
        let mut mesh = egui::Mesh::default();
        // A few intermediate stops give a smoother, eased falloff than a single linear ramp.
        let stops = 6;
        for i in 0..=stops {
            let t = i as f32 / stops as f32;
            let eased = t * t * (3.0 - 2.0 * t);
            let color = super::icons::lerp_color(top, bottom, eased);
            let y = egui::lerp(rect.min.y..=rect.max.y, t);
            mesh.colored_vertex(egui::pos2(rect.min.x, y), color);
            mesh.colored_vertex(egui::pos2(rect.max.x, y), color);
            if i > 0 {
                let b = (i * 2) as u32;
                mesh.add_triangle(b - 2, b - 1, b);
                mesh.add_triangle(b - 1, b + 1, b);
            }
        }
        painter.add(egui::Shape::mesh(mesh));
    }

    /// Install fonts and the global look & feel.
    pub fn install(ctx: &egui::Context) {
        install_fonts(ctx);
        let style = build_style();
        ctx.all_styles_mut(|s| *s = style.clone());
    }
}

fn install_fonts(ctx: &egui::Context) {
    let mut fonts = FontDefinitions::default();

    // Prefer the native Windows UI font when available; egui's bundled fonts stay as fallback
    let fonts_dir = std::env::var("WINDIR")
        .map(|w| std::path::PathBuf::from(w).join("Fonts"))
        .unwrap_or_default();

    let regular = std::fs::read(fonts_dir.join("segoeui.ttf")).ok();
    let semibold = std::fs::read(fonts_dir.join("seguisb.ttf")).ok();

    if let Some(bytes) = regular {
        fonts
            .font_data
            .insert("ui-regular".into(), Arc::new(FontData::from_owned(bytes)));
        fonts
            .families
            .entry(FontFamily::Proportional)
            .or_default()
            .insert(0, "ui-regular".into());
    }

    let mut semibold_family = Vec::new();
    if let Some(bytes) = semibold {
        fonts
            .font_data
            .insert("ui-semibold".into(), Arc::new(FontData::from_owned(bytes)));
        semibold_family.push("ui-semibold".to_string());
    }
    semibold_family.extend(
        fonts
            .families
            .get(&FontFamily::Proportional)
            .cloned()
            .unwrap_or_default(),
    );
    fonts
        .families
        .insert(FontFamily::Name("semibold".into()), semibold_family);

    ctx.set_fonts(fonts);
}

fn build_style() -> Style {
    let mut style = Style::default();

    style.text_styles = [
        (TextStyle::Small, FontId::proportional(11.5)),
        (TextStyle::Body, FontId::proportional(13.5)),
        (TextStyle::Button, FontId::proportional(13.5)),
        (TextStyle::Monospace, FontId::monospace(12.5)),
        (
            TextStyle::Heading,
            FontId::new(18.0, FontFamily::Name("semibold".into())),
        ),
    ]
    .into();

    let spacing = &mut style.spacing;
    spacing.item_spacing = egui::vec2(8.0, 6.0);
    spacing.button_padding = egui::vec2(10.0, 5.0);
    spacing.interact_size = egui::vec2(28.0, 28.0);
    spacing.window_margin = Margin::same(18);
    spacing.menu_margin = Margin::same(6);
    spacing.slider_rail_height = 4.0;
    spacing.slider_width = 160.0;
    spacing.combo_width = 160.0;
    spacing.icon_width = 16.0;
    spacing.icon_width_inner = 8.0;

    let mut v = Visuals::dark();
    v.panel_fill = Theme::BG;
    v.window_fill = Theme::SURFACE;
    v.window_stroke = Stroke::new(1.0, Theme::BORDER);
    v.window_corner_radius = Theme::radius_lg();
    v.window_shadow = Shadow {
        offset: [0, 16],
        blur: 40,
        spread: 0,
        color: Theme::black(160),
    };
    v.popup_shadow = Shadow {
        offset: [0, 10],
        blur: 28,
        spread: 0,
        color: Theme::black(140),
    };
    v.menu_corner_radius = Theme::radius_md();
    v.extreme_bg_color = Theme::CAPSULE_BG;
    v.faint_bg_color = Theme::white(6);
    v.text_edit_bg_color = Some(Theme::CAPSULE_BG);
    v.hyperlink_color = Theme::ACCENT_SOFT;
    v.override_text_color = None;
    v.slider_trailing_fill = true;
    v.handle_shape = egui::style::HandleShape::Circle;
    v.window_highlight_topmost = false;
    v.striped = false;

    v.selection.bg_fill = Theme::ACCENT;
    v.selection.stroke = Stroke::new(1.0, Theme::ACCENT_DARK);

    let radius = Theme::radius_sm();
    let w = &mut v.widgets;

    w.noninteractive.bg_fill = Theme::SURFACE;
    w.noninteractive.weak_bg_fill = Theme::SURFACE;
    w.noninteractive.bg_stroke = Stroke::new(1.0, Theme::BORDER);
    w.noninteractive.fg_stroke = Stroke::new(1.0, Theme::TEXT_MUTED);
    w.noninteractive.corner_radius = radius;

    w.inactive.bg_fill = Theme::white(20);
    w.inactive.weak_bg_fill = Theme::white(10);
    w.inactive.bg_stroke = Stroke::NONE;
    w.inactive.fg_stroke = Stroke::new(1.0, Theme::TEXT);
    w.inactive.corner_radius = radius;
    w.inactive.expansion = 0.0;

    w.hovered.bg_fill = Theme::white(34);
    w.hovered.weak_bg_fill = Theme::white(18);
    w.hovered.bg_stroke = Stroke::NONE;
    w.hovered.fg_stroke = Stroke::new(1.5, Theme::TEXT);
    w.hovered.corner_radius = radius;
    w.hovered.expansion = 0.0;

    w.active.bg_fill = Theme::white(48);
    w.active.weak_bg_fill = Theme::white(26);
    w.active.bg_stroke = Stroke::NONE;
    w.active.fg_stroke = Stroke::new(1.5, Theme::TEXT);
    w.active.corner_radius = radius;
    w.active.expansion = 0.0;

    w.open.bg_fill = Theme::white(22);
    w.open.weak_bg_fill = Theme::white(18);
    w.open.bg_stroke = Stroke::NONE;
    w.open.fg_stroke = Stroke::new(1.0, Theme::TEXT);
    w.open.corner_radius = radius;

    style.visuals = v;
    style
}
