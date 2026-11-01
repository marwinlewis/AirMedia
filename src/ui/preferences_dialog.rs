use super::controls::section_label;
use super::dialog::{setting_row, show_dialog};
use crate::media::decoder::HwAccelMode;
use crate::media::MediaPlayer;
use eframe::egui::{self, Ui};

pub struct PreferencesState {
    pub auto_hide_seconds: f32,
    pub subtitle_size: f32,
    pub default_volume: f32,
}

impl Default for PreferencesState {
    fn default() -> Self {
        Self {
            auto_hide_seconds: 2.5,
            subtitle_size: 20.0,
            default_volume: 1.0,
        }
    }
}

pub struct PreferencesDialog;

impl PreferencesDialog {
    pub fn show(
        ui: &mut Ui,
        player: &mut MediaPlayer,
        prefs: &mut PreferencesState,
        is_open: &mut bool,
    ) {
        show_dialog(ui, "Preferences", is_open, 420.0, |ui, _close| {
            section_label(ui, "Playback");
            setting_row(ui, "Hardware decoder", |ui| {
                egui::ComboBox::from_id_salt("pref_hw")
                    .selected_text(player.hw_accel.name())
                    .width(170.0)
                    .show_ui(ui, |ui| {
                        for mode in HwAccelMode::ALL {
                            if ui.selectable_label(player.hw_accel == mode, mode.name()).clicked() {
                                player.set_hw_accel(mode);
                            }
                        }
                    });
            });
            setting_row(ui, "Startup volume", |ui| {
                let mut pct = (prefs.default_volume * 100.0).round() as u32;
                if ui.add(egui::Slider::new(&mut pct, 0..=150).suffix("%")).changed() {
                    prefs.default_volume = pct as f32 / 100.0;
                }
            });

            ui.add_space(10.0);
            section_label(ui, "Interface");
            setting_row(ui, "Hide controls after", |ui| {
                ui.add(egui::Slider::new(&mut prefs.auto_hide_seconds, 1.0..=10.0).suffix(" s").fixed_decimals(1));
            });

            ui.add_space(10.0);
            section_label(ui, "Subtitles");
            setting_row(ui, "Text size", |ui| {
                ui.add(egui::Slider::new(&mut prefs.subtitle_size, 14.0..=36.0).suffix(" px").fixed_decimals(0));
            });
        });
    }
}
