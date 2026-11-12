use super::dialog::show_dialog;
use super::logo::{LogoCache, LogoVariant};
use super::theme::Theme;
use crate::app_info::{APP_COPYRIGHT, APP_DESCRIPTION, APP_NAME, APP_REPOSITORY, APP_VERSION};
use eframe::egui::{self, RichText, Ui, Vec2};

pub struct AboutDialog;

impl AboutDialog {
    pub fn show(ui: &mut Ui, is_open: &mut bool, logos: &mut LogoCache) {
        show_dialog(ui, "About", is_open, 380.0, |ui, _close| {
            ui.vertical_centered(|ui| {
                let logo_tex = logos.texture(ui.ctx(), LogoVariant::Badge, 56.0);
                let logo_widget = egui::Image::new(&logo_tex)
                    .fit_to_exact_size(Vec2::splat(56.0))
                    .corner_radius(egui::CornerRadius::same(16));
                let resp = ui.add(logo_widget);
                ui.painter().rect_stroke(
                    resp.rect,
                    egui::CornerRadius::same(16),
                    egui::Stroke::new(1.0, Theme::white(25)),
                    Theme::stroke_kind(),
                );

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
