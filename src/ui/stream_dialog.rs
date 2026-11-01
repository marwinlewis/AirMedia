use super::dialog::{footer_row, primary_button, secondary_button, show_dialog};
use super::theme::Theme;
use crate::media::MediaPlayer;
use crate::playlist::Playlist;
use eframe::egui::{self, RichText, Ui};
use std::path::PathBuf;

pub struct StreamDialog;

impl StreamDialog {
    pub fn show(
        ui: &mut Ui,
        player: &mut MediaPlayer,
        playlist: &mut Playlist,
        url_input: &mut String,
        is_open: &mut bool,
    ) {
        show_dialog(ui, "Open network stream", is_open, 440.0, |ui, close| {
            ui.label(RichText::new("HTTP(S), RTSP, RTMP or HLS (.m3u8) URL").color(Theme::TEXT_MUTED));
            ui.add_space(8.0);

            let text_resp = ui.add(
                egui::TextEdit::singleline(url_input)
                    .hint_text("https://example.com/stream.m3u8")
                    .desired_width(f32::INFINITY)
                    .margin(egui::Margin::symmetric(10, 8)),
            );
            text_resp.request_focus();

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
            if play && !url.is_empty() {
                let path = PathBuf::from(url);
                playlist.add(&path);
                playlist.current_index = Some(playlist.items.len() - 1);
                let _ = player.load_file(&path);
                *close = true;
            }
        });
    }
}
