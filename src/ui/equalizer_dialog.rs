use super::controls::section_label;
use super::dialog::{band_slider, footer_row, secondary_button, segmented, show_dialog, toggle};
use super::theme::Theme;
use eframe::egui::{self, Align2, RichText, Ui, Vec2};

#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum EqualizerPreset {
    Flat,
    Rock,
    Pop,
    Classical,
    Jazz,
    Dance,
    BassBoost,
    Vocal,
    Techno,
}

impl EqualizerPreset {
    pub fn name(&self) -> &'static str {
        match self {
            Self::Flat => "Flat",
            Self::Rock => "Rock",
            Self::Pop => "Pop",
            Self::Classical => "Classical",
            Self::Jazz => "Jazz",
            Self::Dance => "Dance",
            Self::BassBoost => "Full Bass",
            Self::Vocal => "Vocal",
            Self::Techno => "Techno",
        }
    }

    pub fn gains(&self) -> [f32; 10] {
        match self {
            Self::Flat => [0.0; 10],
            Self::Rock => [4.8, 3.2, -1.5, -3.0, -1.0, 1.2, 3.5, 4.5, 4.8, 5.0],
            Self::Pop => [-1.5, 1.0, 3.0, 4.0, 3.5, 1.0, -1.0, -1.5, -1.5, -1.5],
            Self::Classical => [4.5, 3.5, 3.0, 2.5, -1.5, -1.5, 0.0, 2.0, 3.5, 4.0],
            Self::Jazz => [3.5, 2.5, 1.0, 1.8, -1.5, -1.5, 0.0, 1.5, 3.0, 3.5],
            Self::Dance => [6.0, 5.0, 2.0, 0.0, 1.5, 3.5, 4.5, 4.0, 3.0, 0.0],
            Self::BassBoost => [7.5, 6.0, 4.5, 2.0, 0.5, 0.0, 0.0, 0.0, 0.0, 0.0],
            Self::Vocal => [-1.0, -2.0, -1.5, 2.0, 4.5, 4.5, 3.5, 1.5, 0.0, -1.5],
            Self::Techno => [5.0, 4.0, 1.0, -2.0, -1.5, 0.5, 4.0, 5.0, 5.0, 4.5],
        }
    }
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
#[serde(default)]
pub struct EqualizerState {
    pub enabled: bool,
    pub preset: EqualizerPreset,
    pub preamp_db: f32,
    pub band_gains_db: [f32; 10],

    /// Which tab of the Effects dialog is shown (UI only, not persisted).
    #[serde(skip)]
    pub show_video_tab: bool,
    // Video adjustments
    pub brightness: f32,
    pub contrast: f32,
    pub saturation: f32,
    pub gamma: f32,
}

impl Default for EqualizerState {
    fn default() -> Self {
        Self {
            enabled: true,
            preset: EqualizerPreset::Flat,
            preamp_db: 0.0,
            band_gains_db: [0.0; 10],
            show_video_tab: false,
            brightness: 0.0,
            contrast: 1.0,
            saturation: 1.0,
            gamma: 1.0,
        }
    }
}

impl EqualizerState {
    /// ffmpeg audio filter chain for the current settings (`None` when disabled or flat).
    pub fn audio_filter(&self) -> Option<String> {
        if !self.enabled {
            return None;
        }
        crate::media::equalizer_filter(self.preamp_db, &self.band_gains_db)
    }

    /// ffmpeg video filter for the color adjustments (`None` at defaults).
    pub fn video_filter(&self) -> Option<String> {
        crate::media::video_adjust_filter(self.brightness, self.contrast, self.saturation, self.gamma)
    }
}

/// What the user committed in the Effects dialog this frame.
#[derive(Debug, Default, Clone, Copy)]
pub struct EffectsChange {
    pub audio: bool,
    pub video: bool,
}

pub struct EqualizerDialog;

const PRESETS: [EqualizerPreset; 9] = [
    EqualizerPreset::Flat,
    EqualizerPreset::Rock,
    EqualizerPreset::Pop,
    EqualizerPreset::Classical,
    EqualizerPreset::Jazz,
    EqualizerPreset::Dance,
    EqualizerPreset::BassBoost,
    EqualizerPreset::Vocal,
    EqualizerPreset::Techno,
];

const BANDS: [&str; 10] = ["31", "63", "125", "250", "500", "1k", "2k", "4k", "8k", "16k"];

impl EqualizerDialog {
    /// Shows the dialog. Changes are reported once a slider is released (or a preset/reset is
    /// picked) because applying them restarts the decoders.
    pub fn show(ui: &mut Ui, state: &mut EqualizerState, is_open: &mut bool) -> EffectsChange {
        let mut change = EffectsChange::default();
        show_dialog(ui, "Effects", is_open, 520.0, |ui, _close| {
            segmented(
                ui,
                &mut state.show_video_tab,
                &[(false, "Equalizer"), (true, "Video")],
            );
            ui.add_space(14.0);

            if !state.show_video_tab {
                change.audio = Self::equalizer(ui, state);
            } else {
                change.video = Self::video(ui, state);
            }
        });
        change
    }

    fn equalizer(ui: &mut Ui, state: &mut EqualizerState) -> bool {
        let mut committed = false;
        ui.horizontal(|ui| {
            if toggle(ui, &mut state.enabled).clicked() {
                committed = true;
            }
            ui.label(RichText::new("Enabled").color(Theme::TEXT));
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                egui::ComboBox::from_id_salt("eq_preset")
                    .selected_text(state.preset.name())
                    .width(130.0)
                    .show_ui(ui, |ui| {
                        for p in PRESETS {
                            if ui.selectable_label(state.preset == p, p.name()).clicked() {
                                state.preset = p;
                                state.band_gains_db = p.gains();
                                committed = true;
                            }
                        }
                    });
            });
        });

        ui.add_space(14.0);

        let enabled = state.enabled;
        ui.add_enabled_ui(enabled, |ui| {
            ui.horizontal(|ui| {
                ui.spacing_mut().item_spacing.x = 10.0;
                committed |= band_column(ui, "Pre", &mut state.preamp_db);
                ui.add_space(6.0);
                let (sep, _) = ui.allocate_exact_size(Vec2::new(1.0, 190.0), egui::Sense::hover());
                ui.painter().rect_filled(sep, egui::CornerRadius::ZERO, Theme::BORDER);
                ui.add_space(6.0);
                for (i, label) in BANDS.iter().enumerate() {
                    committed |= band_column(ui, label, &mut state.band_gains_db[i]);
                }
            });
        });

        ui.add_space(12.0);
        ui.horizontal(|ui| {
            ui.label(RichText::new("Double-click a slider to reset it").size(12.0).color(Theme::TEXT_FAINT));
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                if ui.add(secondary_button("Reset")).clicked() {
                    state.preset = EqualizerPreset::Flat;
                    state.preamp_db = 0.0;
                    state.band_gains_db = [0.0; 10];
                    committed = true;
                }
            });
        });
        committed
    }

    fn video(ui: &mut Ui, state: &mut EqualizerState) -> bool {
        let mut committed = false;
        section_label(ui, "Color");
        egui::Grid::new("video_adjust_grid")
            .num_columns(2)
            .spacing([16.0, 10.0])
            .show(ui, |ui| {
                ui.spacing_mut().slider_width = 320.0;
                for (label, value, range) in [
                    ("Brightness", &mut state.brightness, -1.0..=1.0),
                    ("Contrast", &mut state.contrast, 0.0..=2.0),
                    ("Saturation", &mut state.saturation, 0.0..=3.0),
                    ("Gamma", &mut state.gamma, 0.1..=3.0),
                ] {
                    ui.label(RichText::new(label).color(Theme::TEXT_MUTED));
                    let resp = ui.add(egui::Slider::new(value, range).fixed_decimals(2));
                    // Commit on release, or immediately for keyboard/text edits.
                    if resp.drag_stopped() || (resp.changed() && !resp.dragged()) {
                        committed = true;
                    }
                    ui.end_row();
                }
            });

        ui.add_space(12.0);
        footer_row(ui, |ui| {
            if ui.add(secondary_button("Reset")).clicked() {
                state.brightness = 0.0;
                state.contrast = 1.0;
                state.saturation = 1.0;
                state.gamma = 1.0;
                committed = true;
            }
        });
        committed
    }
}

/// Returns true when the user committed a new value (released the slider or double-clicked).
fn band_column(ui: &mut Ui, label: &str, value: &mut f32) -> bool {
    ui.vertical(|ui| {
        ui.spacing_mut().item_spacing.y = 4.0;
        let (vr, _) = ui.allocate_exact_size(Vec2::new(28.0, 14.0), egui::Sense::hover());
        let txt = if *value > 0.0 { format!("+{value:.0}") } else { format!("{value:.0}") };
        ui.painter().text(vr.center(), Align2::CENTER_CENTER, txt, Theme::font(11.0), Theme::TEXT_MUTED);
        let resp = band_slider(ui, value, -20.0, 20.0, 150.0);
        let (lr, _) = ui.allocate_exact_size(Vec2::new(28.0, 14.0), egui::Sense::hover());
        ui.painter().text(lr.center(), Align2::CENTER_CENTER, label, Theme::font(11.0), Theme::TEXT_FAINT);
        resp.drag_stopped() || resp.clicked() || resp.double_clicked()
    })
    .inner
}
