use super::artwork::{paint_cover, ArtworkCache};
use super::theme::Theme;
use crate::library::MediaKind;
use crate::media::decoder::VideoFrame;
use eframe::egui::{self, Color32, CornerRadius, FontFamily, FontId, Pos2, Rect, Response, Sense, Ui, Vec2};
use std::path::Path;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum AspectRatioMode {
    #[default]
    Fit,
    Fill,
    Stretch,
    Force16_9,
    Force4_3,
    Force21_9,
    Force1_1,
    Original,
}

impl AspectRatioMode {
    pub fn name(&self) -> &'static str {
        match self {
            Self::Fit => "Fit (Letterbox)",
            Self::Fill => "Fill (Crop)",
            Self::Stretch => "Stretch",
            Self::Force16_9 => "16:9",
            Self::Force4_3 => "4:3",
            Self::Force21_9 => "21:9",
            Self::Force1_1 => "1:1",
            Self::Original => "Original (100%)",
        }
    }

    pub fn short_name(&self) -> &'static str {
        match self {
            Self::Fit => "Fit",
            Self::Fill => "Fill",
            Self::Stretch => "Stretch",
            Self::Force16_9 => "16:9",
            Self::Force4_3 => "4:3",
            Self::Force21_9 => "21:9",
            Self::Force1_1 => "1:1",
            Self::Original => "100%",
        }
    }

    pub const ALL: [AspectRatioMode; 8] = [
        AspectRatioMode::Fit,
        AspectRatioMode::Fill,
        AspectRatioMode::Stretch,
        AspectRatioMode::Force16_9,
        AspectRatioMode::Force4_3,
        AspectRatioMode::Force21_9,
        AspectRatioMode::Force1_1,
        AspectRatioMode::Original,
    ];
}

pub struct VideoCanvas {
    pub aspect_mode: AspectRatioMode,
    pub texture: Option<egui::TextureHandle>,
    last_frame_dims: Option<(u32, u32)>,
}

/// What the audio stage shows for the current track.
pub struct AudioStage<'a> {
    pub path: &'a Path,
    pub title: &'a str,
    pub subtitle: &'a str,
    pub levels: &'a [f32; 16],
    /// Space kept free at the bottom for the player controls.
    pub bottom_inset: f32,
}

impl VideoCanvas {
    pub fn new() -> Self {
        Self {
            aspect_mode: AspectRatioMode::Fit,
            texture: None,
            last_frame_dims: None,
        }
    }

    pub fn update_texture(&mut self, ctx: &egui::Context, frame: &VideoFrame) {
        let size = [frame.width as usize, frame.height as usize];
        let color_image = egui::ColorImage::from_rgba_unmultiplied(size, &frame.rgba_data);

        if let Some(tex) = &mut self.texture {
            if self.last_frame_dims == Some((frame.width, frame.height)) {
                tex.set(color_image, egui::TextureOptions::LINEAR);
            } else {
                *tex = ctx.load_texture("video_frame", color_image, egui::TextureOptions::LINEAR);
                self.last_frame_dims = Some((frame.width, frame.height));
            }
        } else {
            self.texture = Some(ctx.load_texture(
                "video_frame",
                color_image,
                egui::TextureOptions::LINEAR,
            ));
            self.last_frame_dims = Some((frame.width, frame.height));
        }
    }

    /// Forgets the last frame so a new file never starts by flashing the previous one.
    pub fn clear(&mut self) {
        self.texture = None;
        self.last_frame_dims = None;
    }

    /// Edge-to-edge video on black. `subtitle_lift` raises subtitles above visible controls.
    pub fn show_video(
        &mut self,
        ui: &mut Ui,
        rect: Rect,
        subtitle_text: Option<&str>,
        subtitle_size: f32,
        subtitle_lift: f32,
    ) -> Response {
        let response = ui.allocate_rect(rect, Sense::click_and_drag());
        let painter = ui.painter_at(rect);
        painter.rect_filled(rect, CornerRadius::ZERO, Color32::BLACK);

        match &self.texture {
            Some(tex) => {
                let (frame_w, frame_h) = self.last_frame_dims.unwrap_or((1920, 1080));
                let target = compute_target_rect(rect, frame_w as f32, frame_h as f32, self.aspect_mode);
                let uv = Rect::from_min_max(Pos2::ZERO, Pos2::new(1.0, 1.0));
                painter.image(tex.id(), target, uv, Color32::WHITE);
            }
            None => {
                // First frame still on its way.
                let spinner = Rect::from_center_size(rect.center(), Vec2::splat(44.0));
                ui.put(spinner, egui::Spinner::new().size(44.0).color(Theme::ACCENT));
            }
        }

        if let Some(sub) = subtitle_text {
            draw_subtitle_overlay(&painter, rect, sub, subtitle_size, subtitle_lift);
        }
        response
    }

    /// The screen for audio-only media: dimmed artwork filling the window, the artwork itself,
    /// the title and a spectrum visualizer.
    pub fn show_audio(&mut self, ui: &mut Ui, rect: Rect, art: &mut ArtworkCache, stage: AudioStage<'_>) -> Response {
        let response = ui.allocate_rect(rect, Sense::click_and_drag());
        let painter = ui.painter_at(rect);
        painter.rect_filled(rect, CornerRadius::ZERO, Theme::BG);

        let tex = art.texture(stage.path);
        if let Some(tex) = &tex {
            paint_cover(&painter, rect, CornerRadius::ZERO, tex, Color32::from_gray(70));
        }
        Theme::paint_vertical_gradient(&painter, rect, Theme::BG.gamma_multiply(0.45), Theme::BG.gamma_multiply(0.97));

        let usable_h = (rect.height() - stage.bottom_inset - 70.0).max(120.0);
        let tile_h = (usable_h - 130.0).clamp(90.0, 340.0).min(rect.width() - 48.0);
        let block_h = tile_h + 92.0;
        let top = rect.min.y + 56.0 + ((usable_h - block_h) / 2.0).max(0.0);
        let tile = Rect::from_center_size(Pos2::new(rect.center().x, top + tile_h / 2.0), Vec2::splat(tile_h));

        painter.rect_filled(tile.translate(Vec2::new(0.0, 10.0)).expand(6.0), Theme::radius_xl(), Theme::black(70));
        match &tex {
            Some(tex) => paint_cover(&painter, tile, Theme::radius_lg(), tex, Color32::WHITE),
            None => art.paint_placeholder(&painter, tile, Theme::radius_lg(), stage.title, MediaKind::Audio),
        }

        let max_w = (rect.width() - 48.0).max(100.0);
        let text_y = tile.max.y + 20.0;
        let title = truncated(&painter, stage.title, Theme::font_bold(24.0), Theme::TEXT, max_w);
        painter.galley(Pos2::new(rect.center().x - title.size().x / 2.0, text_y), title, Theme::TEXT);
        let sub = truncated(&painter, stage.subtitle, Theme::font(14.5), Theme::TEXT_MUTED, max_w);
        painter.galley(Pos2::new(rect.center().x - sub.size().x / 2.0, text_y + 34.0), sub, Theme::TEXT_MUTED);

        draw_visualizer(&painter, rect, stage.levels, stage.bottom_inset);
        response
    }
}

fn truncated(painter: &egui::Painter, text: &str, font: FontId, color: Color32, max_w: f32) -> std::sync::Arc<egui::Galley> {
    let mut job = egui::text::LayoutJob::simple_singleline(text.to_owned(), font, color);
    job.wrap = egui::text::TextWrapping::truncate_at_width(max_w);
    painter.layout_job(job)
}

fn compute_target_rect(
    canvas: Rect,
    video_w: f32,
    video_h: f32,
    mode: AspectRatioMode,
) -> Rect {
    if video_w <= 0.0 || video_h <= 0.0 {
        return canvas;
    }

    let target_aspect = match mode {
        AspectRatioMode::Fit | AspectRatioMode::Fill => video_w / video_h,
        AspectRatioMode::Stretch => return canvas,
        AspectRatioMode::Force16_9 => 16.0 / 9.0,
        AspectRatioMode::Force4_3 => 4.0 / 3.0,
        AspectRatioMode::Force21_9 => 21.0 / 9.0,
        AspectRatioMode::Force1_1 => 1.0,
        AspectRatioMode::Original => {
            let center = canvas.center();
            return Rect::from_center_size(center, Vec2::new(video_w, video_h));
        }
    };

    let canvas_aspect = canvas.width() / canvas.height();

    match mode {
        AspectRatioMode::Fill => {
            if canvas_aspect > target_aspect {
                let h = canvas.width() / target_aspect;
                Rect::from_center_size(canvas.center(), Vec2::new(canvas.width(), h))
            } else {
                let w = canvas.height() * target_aspect;
                Rect::from_center_size(canvas.center(), Vec2::new(w, canvas.height()))
            }
        }
        _ => {
            if canvas_aspect > target_aspect {
                let w = canvas.height() * target_aspect;
                Rect::from_center_size(canvas.center(), Vec2::new(w, canvas.height()))
            } else {
                let h = canvas.width() / target_aspect;
                Rect::from_center_size(canvas.center(), Vec2::new(canvas.width(), h))
            }
        }
    }
}

fn draw_visualizer(painter: &egui::Painter, rect: Rect, levels: &[f32; 16], bottom_inset: f32) {
    if !levels.iter().any(|&l| l > 0.02) {
        return;
    }
    let n = levels.len() as f32;
    let (bar_w, gap) = (5.0, 4.0);
    let start_x = rect.center().x - (n * bar_w + (n - 1.0) * gap) / 2.0;
    let bottom_y = rect.max.y - bottom_inset - 6.0;
    for (i, &lvl) in levels.iter().enumerate() {
        let h = (lvl * 40.0).clamp(2.0, 36.0);
        let x = start_x + i as f32 * (bar_w + gap);
        let bar = Rect::from_min_max(Pos2::new(x, bottom_y - h), Pos2::new(x + bar_w, bottom_y));
        let t = i as f32 / (n - 1.0);
        let color = super::icons::lerp_color(Theme::ACCENT, Theme::ACCENT_SOFT, t);
        painter.rect_filled(bar, Theme::radius_full(), color);
    }
}

fn draw_subtitle_overlay(painter: &egui::Painter, canvas_rect: Rect, text: &str, font_size: f32, lift: f32) {
    if text.trim().is_empty() {
        return;
    }
    let font_id = FontId::new(font_size.clamp(12.0, 48.0), FontFamily::Proportional);
    let max_w = (canvas_rect.width() * 0.8).max(120.0);
    let mut job = egui::text::LayoutJob::simple(text.to_owned(), font_id, Color32::WHITE, max_w);
    job.halign = egui::Align::Center;
    let galley = painter.layout_job(job);

    let pad = Vec2::new(14.0, 6.0);
    let size = galley.size() + pad * 2.0;
    let bottom = canvas_rect.max.y - (canvas_rect.height() * 0.08).max(48.0).max(lift);
    let bg = Rect::from_min_size(Pos2::new(canvas_rect.center().x - size.x / 2.0, bottom - size.y), size);

    painter.rect_filled(bg, Theme::radius_sm(), Theme::black(150));
    painter.galley(Pos2::new(bg.center().x, bg.min.y + pad.y), galley, Color32::WHITE);
}
