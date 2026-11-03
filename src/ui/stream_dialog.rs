use super::dialog::{footer_row, primary_button, secondary_button, show_dialog};
use super::theme::Theme;
use crate::media::is_url;
use eframe::egui::{self, RichText, Ui};
use std::path::PathBuf;

pub struct StreamDialog;

impl StreamDialog {
    /// Returns the URL to open when the user confirms a valid stream address.
    pub fn show(ui: &mut Ui, url_input: &mut String, is_open: &mut bool) -> Option<PathBuf> {
        let mut result = None;
        show_dialog(ui, "Open network stream", is_open, 440.0, |ui, close| {
            ui.label(RichText::new("HTTP(S), RTSP, RTMP or HLS (.m3u8) URL").color(Theme::TEXT_MUTED));
            ui.add_space(8.0);

            let text_resp = ui.add(
                egui::TextEdit::singleline(url_input)
                    .hint_text("https://example.com/stream.m3u8")
                    .desired_width(f32::INFINITY)
                    .margin(egui::Margin::symmetric(10, 8)),
            );
            // Only grab focus when nothing else has it, so Tab can reach the buttons.
            if ui.memory(|m| m.focused().is_none()) {
                text_resp.request_focus();
            }

            ui.add_space(16.0);

            let mut play = text_resp.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter));
            footer_row(ui, |ui| {
                let can_play = !url_input.trim().is_empty();
                if ui.add_enabled(can_play, primary_button("Play")).clicked() {
                    play = true;
                }
                if ui.add(secondary_button("Cancel")).clicked() {
                    *close = true;
                }
            });

            let url = url_input.trim();
            let valid = is_url(&PathBuf::from(url));
            if !url.is_empty() && !valid {
                ui.label(
                    RichText::new("Enter a full URL including the scheme, e.g. https://")
                        .size(12.0)
                        .color(Theme::TEXT_FAINT),
                );
            }
            if play && valid {
                result = Some(PathBuf::from(url));
                *close = true;
            }
        });
        result
    }
}
