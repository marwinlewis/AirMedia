use super::controls::section_label;
use super::dialog::{setting_row, show_dialog};
use crate::media::decoder::HwAccelMode;
use crate::media::{default_pictures_dir, MediaPlayer, ScreenshotFormat};
use eframe::egui::{self, Ui};
use std::path::PathBuf;

#[derive(Clone, serde::Serialize, serde::Deserialize)]
#[serde(default)]
pub struct PreferencesState {
    pub auto_hide_seconds: f32,
    pub subtitle_size: f32,
    pub default_volume: f32,
    pub screenshot_dir: PathBuf,
    pub screenshot_format: ScreenshotFormat,
    pub hw_accel: HwAccelMode,
    /// Keep the player window above other windows (Ctrl+T / app menu).
    pub always_on_top: bool,
}

impl Default for PreferencesState {
    fn default() -> Self {
        Self {
            auto_hide_seconds: 2.5,
            subtitle_size: 20.0,
            default_volume: 1.0,
            screenshot_dir: default_pictures_dir(),
            screenshot_format: ScreenshotFormat::Jpg,
            hw_accel: HwAccelMode::Auto,
            always_on_top: false,
        }
    }
}

impl PreferencesState {
    pub fn config_path() -> Option<PathBuf> {
        crate::persistence::config_dir().map(|d| d.join("preferences.json"))
    }

    pub fn load() -> Self {
        if let Some(path) = Self::config_path() {
            if let Ok(content) = std::fs::read_to_string(&path) {
                if let Ok(prefs) = serde_json::from_str::<Self>(crate::persistence::strip_bom(&content)) {
                    return prefs;
                }
            }
        }
        Self::default()
    }

    pub fn save(&self) {
        if let Some(path) = Self::config_path() {
            if let Ok(json) = serde_json::to_string_pretty(self) {
                if let Err(e) = crate::persistence::write_atomic(&path, &json) {
                    eprintln!("Failed to save preferences to {}: {e}", path.display());
                }
            }
        }
    }
}

/// Keeps the last `max_chars - 1` characters of `s` behind an ellipsis. Works on chars, not
/// bytes, so non-ASCII folder names can't split a UTF-8 sequence.
pub fn shorten_path_label(s: &str, max_chars: usize) -> String {
    let count = s.chars().count();
    if count <= max_chars {
        return s.to_string();
    }
    let keep = max_chars.saturating_sub(1);
    let tail: String = s.chars().skip(count - keep).collect();
    format!("…{tail}")
}

pub struct PreferencesDialog;

impl PreferencesDialog {
    pub fn show(
        ui: &mut Ui,
        player: &mut MediaPlayer,
        prefs: &mut PreferencesState,
        is_open: &mut bool,
    ) {
        show_dialog(ui, "Preferences", is_open, 440.0, |ui, _close| {
            let mut changed = false;

            section_label(ui, "Playback");
            setting_row(ui, "Hardware decoder", |ui| {
                egui::ComboBox::from_id_salt("pref_hw")
                    .selected_text(player.hw_accel.name())
                    .width(170.0)
                    .show_ui(ui, |ui| {
                        for mode in HwAccelMode::ALL {
                            if ui.selectable_label(player.hw_accel == mode, mode.name()).clicked() {
                                player.set_hw_accel(mode);
                                prefs.hw_accel = mode;
                                changed = true;
                            }
                        }
                    });
            });
            setting_row(ui, "Startup volume", |ui| {
                let mut pct = (prefs.default_volume * 100.0).round() as u32;
                if ui.add(egui::Slider::new(&mut pct, 0..=150).suffix("%")).changed() {
                    prefs.default_volume = pct as f32 / 100.0;
                    changed = true;
                }
            });

            ui.add_space(10.0);
            section_label(ui, "Interface");
            setting_row(ui, "Hide controls after", |ui| {
                if ui.add(egui::Slider::new(&mut prefs.auto_hide_seconds, 1.0..=10.0).suffix(" s").fixed_decimals(1)).changed() {
                    changed = true;
                }
            });

            ui.add_space(10.0);
            section_label(ui, "Subtitles");
            setting_row(ui, "Text size", |ui| {
                if ui.add(egui::Slider::new(&mut prefs.subtitle_size, 14.0..=36.0).suffix(" px").fixed_decimals(0)).changed() {
                    changed = true;
                }
            });

            ui.add_space(10.0);
            section_label(ui, "Screenshots");
            setting_row(ui, "Save folder", |ui| {
                let dir_str = prefs.screenshot_dir.to_string_lossy().to_string();
                let short_str = shorten_path_label(&dir_str, 22);
                let current_dir = prefs.screenshot_dir.clone();

                ui.horizontal(|ui| {
                    if ui.button(format!("📁 {short_str}"))
                        .on_hover_text(format!("Click to change folder\nCurrent: {dir_str}"))
                        .clicked()
                    {
                        if let Some(folder) = crate::ui::FileDialog::new()
                            .set_directory(&current_dir)
                            .pick_folder()
                        {
                            prefs.screenshot_dir = folder;
                            changed = true;
                        }
                    }
                    if ui.button("Reset").on_hover_text("Reset to default Pictures folder").clicked() {
                        prefs.screenshot_dir = default_pictures_dir();
                        changed = true;
                    }
                });
            });

            setting_row(ui, "Format", |ui| {
                egui::ComboBox::from_id_salt("pref_screenshot_fmt")
                    .selected_text(prefs.screenshot_format.name())
                    .width(150.0)
                    .show_ui(ui, |ui| {
                        for fmt in ScreenshotFormat::ALL {
                            if ui.selectable_label(prefs.screenshot_format == fmt, fmt.name()).clicked() {
                                prefs.screenshot_format = fmt;
                                changed = true;
                            }
                        }
                    });
            });

            if changed {
                prefs.save();
            }
        });
    }
}
