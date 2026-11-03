use super::icons::{icon_button, Icon};
use super::theme::Theme;
use crate::media::MediaPlayer;
use eframe::egui::{self, Pos2, Rect, Response, RichText, Ui, Vec2};

pub const TOP_BAR_HEIGHT: f32 = 96.0;

pub struct TopBarView;

impl TopBarView {
    /// Paints the top overlay inside `area` and returns the app-menu button response.
    pub fn show(ui: &mut Ui, area: Rect, player: &MediaPlayer, title: &str) -> Response {
        let scrim = Rect::from_min_max(area.min, Pos2::new(area.max.x, area.min.y + TOP_BAR_HEIGHT));
        Theme::paint_vertical_gradient(ui.painter(), scrim, Theme::black(170), Theme::black(0));

        let row_rect = Rect::from_min_size(area.min + Vec2::new(12.0, 10.0), Vec2::new(area.width() - 24.0, 40.0));
        let mut row = ui.new_child(
            egui::UiBuilder::new()
                .max_rect(row_rect)
                .layout(egui::Layout::left_to_right(egui::Align::Center)),
        );

        let menu_resp = icon_button(&mut row, Icon::Menu, 36.0).on_hover_text("Menu");
        row.add_space(6.0);

        // Right-side technical details, rendered quietly.
        let mut details = Vec::new();
        if let Some(info) = &player.media_info {
            if let Some((w, h)) = info.video_dimensions() {
                details.push(format!("{w} × {h}"));
                let fps = info.fps();
                if fps > 0.0 {
                    details.push(format!("{fps:.0} fps"));
                }
            }
            details.push(player.effective_hw_accel().short_name().to_string());
        }

        row.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            if !details.is_empty() {
                ui.label(RichText::new(details.join("  ·  ")).size(12.0).color(Theme::TEXT_MUTED));
                ui.add_space(12.0);
            }
            ui.with_layout(egui::Layout::left_to_right(egui::Align::Center), |ui| {
                if !title.is_empty() {
                    ui.add(
                        egui::Label::new(
                            RichText::new(title)
                                .font(Theme::font_semibold(14.5))
                                .color(Theme::TEXT),
                        )
                        .truncate()
                        .selectable(false),
                    );
                }
            });
        });

        menu_resp
    }
}
