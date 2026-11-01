use super::theme::Theme;
use crate::media::decoder::VideoFrame;
use eframe::egui::{
    self, Color32, FontFamily, FontId, Pos2, Rect, Response, Sense, Ui, Vec2,
};

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

    pub fn show(
        &mut self,
        ui: &mut Ui,
        subtitle_text: Option<&str>,
        subtitle_size: f32,
        is_audio_only: bool,
        visualizer_levels: &[f32; 16],
    ) -> Response {
        let available_size = ui.available_size();
        let (rect, response) =
            ui.allocate_exact_size(available_size, Sense::click_and_drag());

        let painter = ui.painter_at(rect);

        // Fill background
        painter.rect_filled(rect, egui::CornerRadius::ZERO, Theme::BG);

        if let (Some(tex), false) = (&self.texture, is_audio_only) {
            let (frame_w, frame_h) = self.last_frame_dims.unwrap_or((1920, 1080));
            let target_rect = compute_target_rect(
                rect,
                frame_w as f32,
                frame_h as f32,
                self.aspect_mode,
            );

            // Render texture with GPU filtering
            let uv = Rect::from_min_max(Pos2::new(0.0, 0.0), Pos2::new(1.0, 1.0));
            painter.image(tex.id(), target_rect, uv, Color32::WHITE);
        } else {
            draw_idle_or_audio_canvas(&painter, rect, is_audio_only, visualizer_levels);
        }

        if let Some(sub) = subtitle_text {
            draw_subtitle_overlay(&painter, rect, sub, subtitle_size);
        }

        response
    }
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

fn draw_subtitle_overlay(painter: &egui::Painter, canvas_rect: Rect, text: &str, font_size: f32) {
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
    let bottom = canvas_rect.max.y - (canvas_rect.height() * 0.08).max(48.0);
    let bg = Rect::from_min_size(Pos2::new(canvas_rect.center().x - size.x / 2.0, bottom - size.y), size);

    painter.rect_filled(bg, Theme::radius_sm(), Theme::black(150));
    // With centered halign the galley is laid out around x = 0, so anchor it at the box center.
    painter.galley(Pos2::new(bg.center().x, bg.min.y + pad.y), galley, Color32::WHITE);
}

fn draw_idle_or_audio_canvas(
    painter: &egui::Painter,
    rect: Rect,
    is_audio: bool,
    levels: &[f32; 16],
) {
    if !is_audio {
        // The empty state (logo + actions) is drawn by the app on top of the canvas.
        return;
    }

    let center = rect.center();
    let bar_w = 8.0;
    let gap = 6.0;
    let n = levels.len();
    let total_w = n as f32 * bar_w + (n as f32 - 1.0) * gap;
    let start_x = center.x - total_w / 2.0;

    for (i, &lvl) in levels.iter().enumerate() {
        let h = (lvl * 160.0).clamp(6.0, 150.0);
        let x = start_x + i as f32 * (bar_w + gap);
        let bar = Rect::from_min_max(
            Pos2::new(x, center.y - h / 2.0),
            Pos2::new(x + bar_w, center.y + h / 2.0),
        );
        let t = i as f32 / (n - 1) as f32;
        let color = super::icons::lerp_color(Theme::ACCENT, Theme::ACCENT_SOFT, t);
        painter.rect_filled(bar, Theme::radius_full(), color);
    }
}
