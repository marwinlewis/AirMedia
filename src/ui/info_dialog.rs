use super::controls::{format_time, section_label};
use super::dialog::show_dialog;
use super::theme::Theme;
use crate::media::MediaPlayer;
use eframe::egui::{self, RichText, Ui};

pub struct InfoDialog;

fn rows(ui: &mut Ui, id: &str, rows: &[(&str, String)]) {
    egui::Grid::new(id)
        .num_columns(2)
        .spacing([24.0, 8.0])
        .min_row_height(18.0)
        .min_col_width(110.0)
        .show(ui, |ui| {
            for (k, v) in rows {
                ui.label(RichText::new(*k).color(Theme::TEXT_FAINT));
                ui.add(egui::Label::new(RichText::new(v).color(Theme::TEXT)).wrap());
                ui.end_row();
            }
        });
}

fn kbps(s: &Option<String>) -> Option<String> {
    s.as_ref()?.parse::<u64>().ok().map(|b| format!("{} kbps", b / 1000))
}

impl InfoDialog {
    pub fn show(ui: &mut Ui, player: &MediaPlayer, is_open: &mut bool) {
        show_dialog(ui, "Media information", is_open, 460.0, |ui, _close| {
            let Some(info) = &player.media_info else {
                ui.label(RichText::new("No media is open.").color(Theme::TEXT_MUTED));
                return;
            };

            egui::ScrollArea::vertical().max_height(460.0).show(ui, |ui| {
                let mut general = Vec::new();
                if let Some(path) = &player.current_file {
                    general.push(("Location", path.display().to_string()));
                }
                general.push(("Container", info.format.format_long_name.clone()));
                general.push(("Duration", format_time(info.duration_seconds())));
                if let Some(bytes) = info.format.size.as_ref().and_then(|s| s.parse::<u64>().ok()) {
                    general.push(("Size", format!("{:.1} MB", bytes as f64 / (1024.0 * 1024.0))));
                }
                if let Some(b) = kbps(&info.format.bit_rate) {
                    general.push(("Bitrate", b));
                }
                section_label(ui, "General");
                rows(ui, "info_general", &general);

                if let Some(v) = info.primary_video_stream() {
                    let mut video = Vec::new();
                    let codec = v.codec_name.as_deref().unwrap_or("unknown");
                    video.push(("Codec", v.codec_long_name.clone().unwrap_or_else(|| codec.to_string())));
                    if let (Some(w), Some(h)) = (v.width, v.height) {
                        video.push(("Resolution", format!("{w} × {h}")));
                    }
                    video.push(("Frame rate", format!("{:.2} fps", info.fps())));
                    if let Some(b) = kbps(&v.bit_rate) {
                        video.push(("Bitrate", b));
                    }
                    video.push(("Decoder", player.effective_hw_accel().name().to_string()));
                    ui.add_space(12.0);
                    section_label(ui, "Video");
                    rows(ui, "info_video", &video);
                }

                for (idx, a) in info.audio_streams().iter().enumerate() {
                    let mut audio = Vec::new();
                    let codec = a.codec_name.as_deref().unwrap_or("unknown");
                    audio.push(("Codec", a.codec_long_name.clone().unwrap_or_else(|| codec.to_string())));
                    if let Some(sr) = &a.sample_rate {
                        audio.push(("Sample rate", format!("{sr} Hz")));
                    }
                    if let Some(ch) = a.channels {
                        let layout = a.channel_layout.as_deref().map(|l| format!(" ({l})")).unwrap_or_default();
                        audio.push(("Channels", format!("{ch}{layout}")));
                    }
                    if let Some(b) = kbps(&a.bit_rate) {
                        audio.push(("Bitrate", b));
                    }
                    ui.add_space(12.0);
                    let active = if player.active_audio_track == idx { " · active" } else { "" };
                    section_label(ui, &format!("Audio {}{active}", idx + 1));
                    rows(ui, &format!("info_audio_{idx}"), &audio);
                }

                for (idx, sub) in player.subtitles.iter().enumerate() {
                    let source = if sub.is_external { "External file" } else { "Embedded" };
                    let data = [
                        ("Name", sub.name.clone()),
                        ("Language", sub.language.clone()),
                        ("Source", source.to_string()),
                        ("Cues", sub.cues.len().to_string()),
                    ];
                    ui.add_space(12.0);
                    let active = if player.active_subtitle_track == Some(idx) { " · active" } else { "" };
                    section_label(ui, &format!("Subtitle {}{active}", idx + 1));
                    rows(ui, &format!("info_sub_{idx}"), &data);
                }
            });
        });
    }
}
