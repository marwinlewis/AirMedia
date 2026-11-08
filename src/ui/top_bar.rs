use super::icons::{icon_button, Icon};
use super::theme::Theme;
use crate::media::MediaPlayer;
use eframe::egui::{self, Align2, Pos2, Rect, Response, Ui, Vec2};

pub const TOP_BAR_HEIGHT: f32 = 56.0;

pub struct TopBarResponse {
    pub menu_response: Response,
    pub search_clicked: bool,
}

pub struct TopBarView;

impl TopBarView {
    /// Paints the top bar matching the design:
    /// - Hamburger Menu button on the left
    /// - "Winamp" header title centered
    /// - Search icon button on the right
    pub fn show(ui: &mut Ui, area: Rect, _player: &MediaPlayer, _title: &str) -> TopBarResponse {
        let row_rect = Rect::from_min_size(
            Pos2::new(area.min.x + 12.0, area.min.y + 8.0),
            Vec2::new(area.width() - 24.0, 44.0),
        );

        let mut row = ui.new_child(
            egui::UiBuilder::new()
                .max_rect(row_rect)
                .layout(egui::Layout::left_to_right(egui::Align::Center)),
        );

        // Left: Hamburger Menu
        let menu_response = icon_button(&mut row, Icon::Menu, 34.0).on_hover_text("Menu");

        // Center: "Winamp"
        ui.painter().text(
            row_rect.center(),
            Align2::CENTER_CENTER,
            "Winamp",
            Theme::font_bold(18.5),
            Theme::TEXT,
        );

        // Right: Search Icon
        let mut search_clicked = false;
        row.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            if icon_button(ui, Icon::Search, 34.0).on_hover_text("Search Playlist (L)").clicked() {
                search_clicked = true;
            }
        });

        TopBarResponse {
            menu_response,
            search_clicked,
        }
    }
}
