use super::dialog::show_dialog;
use super::icons::Icon;
use super::theme::Theme;
use crate::app_info::{APP_COPYRIGHT, APP_DESCRIPTION, APP_NAME, APP_REPOSITORY, APP_VERSION};
use eframe::egui::{self, RichText, Ui, Vec2};

pub struct AboutDialog;

impl AboutDialog {
    pub fn show(ui: &mut Ui, is_open: &mut bool) {
        show_dialog(ui, "About", is_open, 380.0, |ui, _close| {
            ui.vertical_centered(|ui| {
                let (logo, _) = ui.allocate_exact_size(Vec2::splat(56.0), egui::Sense::hover());
                ui.painter().rect_filled(logo, egui::CornerRadius::same(16), Theme::ACCENT);
                Icon::Play.paint(ui.painter(), logo.shrink(16.0).translate(Vec2::new(2.0, 0.0)), Theme::TEXT);

                ui.add_space(12.0);
                ui.label(RichText::new(APP_NAME).font(Theme::font_semibold(22.0)).color(Theme::TEXT));
                ui.label(
                    RichText::new(format!("Version {APP_VERSION}"))
                        .size(12.0)
                        .color(Theme::TEXT_FAINT),
                );
                ui.add_space(8.0);
                ui.label(
                    RichText::new(APP_DESCRIPTION)
                        .size(12.5)
                        .color(Theme::TEXT_MUTED),
                );
                ui.add_space(6.0);
                ui.label(
                    RichText::new(APP_COPYRIGHT)
                        .size(11.0)
                        .color(Theme::TEXT_FAINT),
                );
                ui.add_space(6.0);
                ui.hyperlink_to(
                    RichText::new("GitHub Repository").size(12.0).color(Theme::ACCENT),
                    APP_REPOSITORY,
                );
                ui.add_space(14.0);
            });

            for (k, v) in [
                ("Rendering", "wgpu · DirectX 12 / 11 · Vulkan"),
                ("Decoding", "D3D11VA · D3D12VA · CUDA · Vulkan"),
                ("Audio", "WASAPI via cpal"),
                ("Subtitles", "Embedded, SRT, VTT"),
            ] {
                ui.horizontal(|ui| {
                    ui.label(RichText::new(k).color(Theme::TEXT_FAINT).size(12.5));
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        ui.label(RichText::new(v).color(Theme::TEXT_MUTED).size(12.5));
                    });
                });
            }
        });
    }
}
