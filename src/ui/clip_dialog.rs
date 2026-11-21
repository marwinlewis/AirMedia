//! Clip export dialog with format/mode selection, folder picker, overwrite protection, and progress display.

use crate::clip::{default_clip_filename, ClipExportMode, ClipRange};
use crate::media::clip_export::ClipExportJob;
use crate::ui::dialog::{footer_row, primary_button, secondary_button, setting_row, show_dialog};
use crate::ui::preferences_dialog::{shorten_path_label, PreferencesState};
use crate::ui::theme::Theme;
use eframe::egui::{self, ProgressBar, RichText, Ui};
use std::path::{Path, PathBuf};

/// State preserved across frames for the export dialog.
pub struct ClipDialogState {
    pub filename: String,
    pub folder: PathBuf,
    pub mode: ClipExportMode,
    pub include_audio: bool,
    pub confirming_overwrite: bool,
    pub error_msg: Option<String>,
    /// Set when the last export finished; the dialog then shows where the clip went.
    pub saved_path: Option<PathBuf>,
}

impl ClipDialogState {
    pub fn new(current_file: &Path, range: ClipRange, prefs: &PreferencesState) -> Self {
        let filename = default_clip_filename(current_file, range);
        let folder = prefs
            .last_clip_folder
            .clone()
            .filter(|p| p.exists())
            .or_else(|| current_file.parent().map(|p| p.to_path_buf()))
            .unwrap_or_else(crate::media::default_pictures_dir);
        let mode = prefs.last_clip_mode;
        let filename = if mode == ClipExportMode::Precise { precise_filename(&filename) } else { filename };
        Self {
            filename,
            folder,
            mode,
            include_audio: true,
            confirming_overwrite: false,
            error_msg: None,
            saved_path: None,
        }
    }
}

pub struct ClipDialog;

impl ClipDialog {
    pub fn show(
        ui: &Ui,
        state: &mut ClipDialogState,
        is_open: &mut bool,
        current_file: Option<&Path>,
        range: ClipRange,
        prefs: &mut PreferencesState,
        clip_export: &mut Option<ClipExportJob>,
    ) {
        show_dialog(ui, "Export Clip", is_open, 460.0, |ui, close| {
            if let Some(saved) = state.saved_path.clone() {
                ui.label(
                    RichText::new("Clip saved")
                        .font(Theme::font_semibold(14.5))
                        .color(Theme::TEXT),
                );
                ui.add_space(4.0);
                ui.label(
                    RichText::new(shorten_path_label(&saved.to_string_lossy(), 56))
                        .font(Theme::font(12.0))
                        .color(Theme::TEXT_MUTED),
                )
                .on_hover_text(saved.to_string_lossy());
                ui.add_space(16.0);
                footer_row(ui, |ui| {
                    if ui.add(primary_button("Done")).clicked() {
                        *close = true;
                    }
                    #[cfg(not(target_os = "android"))]
                    if ui.add(secondary_button("Show in folder")).clicked() {
                        crate::media::clip_export::show_in_folder(&saved);
                    }
                });
            } else if let Some(job) = clip_export.as_mut() {
                // Export is running in background
                ui.label(
                    RichText::new("Exporting video clip\u{2026}")
                        .font(Theme::font_semibold(14.5))
                        .color(Theme::TEXT),
                );
                ui.add_space(4.0);
                ui.label(
                    RichText::new(state.mode.name())
                        .font(Theme::font(12.0))
                        .color(Theme::TEXT_MUTED),
                );
                let target_path = state.folder.join(&state.filename);
                let short_dest = shorten_path_label(&target_path.to_string_lossy(), 42);
                ui.label(
                    RichText::new(format!("Saving to: {short_dest}"))
                        .font(Theme::font(11.5))
                        .color(Theme::TEXT_FAINT),
                );
                ui.add_space(12.0);

                let progress = job.progress();
                ui.add(ProgressBar::new(progress).show_percentage());
                ui.add_space(16.0);

                footer_row(ui, |ui| {
                    if ui.add(secondary_button("Cancel")).clicked() {
                        job.cancel();
                        *close = true;
                    }
                });
            } else {
                // Settings configuration
                setting_row(ui, "Filename", |ui| {
                    ui.add(
                        egui::TextEdit::singleline(&mut state.filename)
                            .desired_width(260.0)
                            .margin(egui::Margin::symmetric(8, 6)),
                    );
                });

                setting_row(ui, "Save folder", |ui| {
                    let dir_str = state.folder.to_string_lossy().to_string();
                    let short_str = shorten_path_label(&dir_str, 20);
                    let cur_folder = state.folder.clone();

                    ui.horizontal(|ui| {
                        if ui
                            .button(format!("📁 {short_str}"))
                            .on_hover_text(format!("Click to choose folder\nCurrent: {dir_str}"))
                            .clicked()
                        {
                            if let Some(folder) = crate::ui::FileDialog::new()
                                .set_directory(&cur_folder)
                                .pick_folder()
                            {
                                state.folder = folder.clone();
                                state.confirming_overwrite = false;
                                prefs.last_clip_folder = Some(folder);
                                prefs.save();
                            }
                        }
                    });
                });

                setting_row(ui, "Export mode", |ui| {
                    egui::ComboBox::from_id_salt("clip_export_mode_combo")
                        .selected_text(state.mode.short_name())
                        .width(180.0)
                        .show_ui(ui, |ui| {
                            for m in ClipExportMode::ALL {
                                if ui.selectable_label(state.mode == m, m.name()).clicked() {
                                    state.mode = m;
                                    if m == ClipExportMode::Precise {
                                        state.filename = precise_filename(&state.filename);
                                    }
                                    state.error_msg = None;
                                    prefs.last_clip_mode = m;
                                    prefs.save();
                                }
                            }
                        });
                });

                setting_row(ui, "Audio", |ui| {
                    ui.checkbox(&mut state.include_audio, "Include audio");
                });

                if let Some(ref err) = state.error_msg {
                    ui.add_space(6.0);
                    ui.label(RichText::new(err).size(12.0).color(Theme::TEXT_FAINT));
                }

                ui.add_space(14.0);

                let mut trigger_start = false;

                if state.confirming_overwrite {
                    egui::Frame::NONE
                        .fill(Theme::white(14))
                        .corner_radius(Theme::radius_sm())
                        .inner_margin(egui::Margin::same(10))
                        .show(ui, |ui| {
                            ui.horizontal(|ui| {
                                ui.label(
                                    RichText::new("⚠️")
                                        .font(Theme::font_semibold(15.0))
                                        .color(Theme::ACCENT),
                                );
                                ui.label(
                                    RichText::new(
                                        "A file with this name already exists. Overwrite?",
                                    )
                                    .font(Theme::font_semibold(12.5))
                                    .color(Theme::TEXT),
                                );
                            });
                        });
                    ui.add_space(10.0);

                    footer_row(ui, |ui| {
                        if ui.add(primary_button("Overwrite")).clicked() {
                            state.confirming_overwrite = false;
                            trigger_start = true;
                        }
                        if ui.add(secondary_button("Cancel")).clicked() {
                            state.confirming_overwrite = false;
                        }
                    });
                } else {
                    footer_row(ui, |ui| {
                        let can_export =
                            !state.filename.trim().is_empty() && current_file.is_some();
                        if ui
                            .add_enabled(can_export, primary_button("Export"))
                            .clicked()
                        {
                            match current_file.map(|src| check_output(src, state)) {
                                Some(Ok(target_path)) => {
                                    state.error_msg = None;
                                    if target_path.exists() {
                                        state.confirming_overwrite = true;
                                    } else {
                                        trigger_start = true;
                                    }
                                }
                                Some(Err(e)) => state.error_msg = Some(e),
                                None => {}
                            }
                        }
                        if ui.add(secondary_button("Cancel")).clicked() {
                            *close = true;
                        }
                    });
                }

                if trigger_start {
                    let checked = current_file.map(|src| (src, check_output(src, state)));
                    if let Some((_, Err(e))) = &checked {
                        state.error_msg = Some(e.clone());
                    }
                    if let Some((src, Ok(target_path))) = checked {
                        prefs.last_clip_folder = Some(state.folder.clone());
                        prefs.last_clip_mode = state.mode;
                        prefs.save();

                        match ClipExportJob::start(
                            src,
                            &target_path,
                            range,
                            state.mode,
                            state.include_audio,
                        ) {
                            Ok(job) => {
                                *clip_export = Some(job);
                                state.error_msg = None;
                            }
                            Err(e) => {
                                state.error_msg = Some(format!("Failed to start export: {e}"));
                            }
                        }
                    }
                }
            }
        });

        // If dialog was closed while exporting, cancel the background export
        if !*is_open {
            if let Some(mut job) = clip_export.take() {
                job.cancel();
            }
            state.confirming_overwrite = false;
        }
    }
}

/// Validates the dialog's filename for the current mode and returns the output path.
fn check_output(src: &Path, state: &ClipDialogState) -> Result<PathBuf, String> {
    let out = crate::clip::validate_clip_output(src, &state.folder, &state.filename)?;
    if state.mode == ClipExportMode::Precise {
        let ext = out
            .extension()
            .and_then(|e| e.to_str())
            .map(str::to_ascii_lowercase)
            .unwrap_or_default();
        if !crate::clip::PRECISE_EXTENSIONS.contains(&ext.as_str()) {
            return Err(format!("Precise mode can't write .{ext} files; use .mp4 or .mkv"));
        }
    }
    Ok(out)
}

/// Swaps an extension Precise mode can't write (e.g. `.ogv`, `.wmv`) for `.mp4`.
fn precise_filename(name: &str) -> String {
    let path = Path::new(name);
    let ext = path
        .extension()
        .and_then(|e| e.to_str())
        .map(str::to_ascii_lowercase)
        .unwrap_or_default();
    if crate::clip::PRECISE_EXTENSIONS.contains(&ext.as_str()) {
        name.to_string()
    } else {
        path.with_extension("mp4").to_string_lossy().into_owned()
    }
}
