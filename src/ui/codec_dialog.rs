//! Dialog displaying Codec Status (FFmpeg / FFprobe) and offering on-the-fly download if missing.

use crate::media::dependency::{check_dependencies, CodecDownloader, DependencyStatus, DownloadState};
use super::dialog::{footer_row, primary_button, secondary_button, show_dialog};
use super::theme::Theme;
use eframe::egui::{self, Color32, ProgressBar, RichText, Ui};

pub struct CodecDialogState {
    pub status: DependencyStatus,
    pub downloader: CodecDownloader,
    /// Whether `status` was already refreshed after the last completed download, so the
    /// (process-spawning) dependency check runs once instead of every frame.
    refreshed_after_download: bool,
}

impl Default for CodecDialogState {
    fn default() -> Self {
        Self {
            status: check_dependencies(),
            downloader: CodecDownloader::new(),
            refreshed_after_download: false,
        }
    }
}

pub struct CodecDialog;

impl CodecDialog {
    pub fn show(ui: &Ui, state: &mut CodecDialogState, open: &mut bool) {
        // Auto-refresh when download completes
        let dl_state = state.downloader.state();
        match dl_state {
            DownloadState::Completed(_) if !state.refreshed_after_download => {
                state.status = check_dependencies();
                state.refreshed_after_download = true;
            }
            DownloadState::Downloading(_) | DownloadState::Extracting => {
                state.refreshed_after_download = false;
            }
            _ => {}
        }

        show_dialog(ui, "Media Engine Codecs (FFmpeg / FFprobe)", open, 540.0, |ui, close| {
            // Status banner
            if state.status.is_ready() {
                egui::Frame::NONE
                    .fill(Color32::from_rgb(18, 52, 28))
                    .corner_radius(Theme::radius_md())
                    .inner_margin(egui::Margin::same(12))
                    .show(ui, |ui| {
                        ui.horizontal(|ui| {
                            ui.label(RichText::new("✔").font(Theme::font_semibold(18.0)).color(Color32::from_rgb(72, 199, 116)));
                            ui.vertical(|ui| {
                                ui.label(
                                    RichText::new("Codec Engine Ready")
                                        .font(Theme::font_semibold(14.0))
                                        .color(Color32::WHITE),
                                );
                                let mode_msg = if state.status.is_preinstalled() {
                                    "Using preinstalled system FFmpeg codecs (Zero download required)."
                                } else {
                                    "Using codecs installed for AirMedia."
                                };
                                ui.label(RichText::new(mode_msg).font(Theme::font(12.0)).color(Theme::TEXT_MUTED));
                            });
                        });
                    });
            } else {
                egui::Frame::NONE
                    .fill(Color32::from_rgb(58, 32, 18))
                    .corner_radius(Theme::radius_md())
                    .inner_margin(egui::Margin::same(12))
                    .show(ui, |ui| {
                        ui.horizontal(|ui| {
                            ui.label(RichText::new("⚠").font(Theme::font_semibold(18.0)).color(Color32::from_rgb(255, 180, 50)));
                            ui.vertical(|ui| {
                                ui.label(
                                    RichText::new("Prerequisite Missing: FFmpeg / FFprobe")
                                        .font(Theme::font_semibold(14.0))
                                        .color(Color32::WHITE),
                                );
                                ui.label(
                                    RichText::new(
                                        "AirMedia relies on FFmpeg and FFprobe for hardware-accelerated decoding. "
                                    )
                                    .font(Theme::font(12.0))
                                    .color(Theme::TEXT_MUTED),
                                );
                            });
                        });
                    });
            }

            ui.add_space(14.0);

            // Detailed status list
            ui.label(RichText::new("DETECTED COMPONENTS").font(Theme::font_semibold(11.0)).color(Theme::TEXT_FAINT));
            ui.add_space(6.0);

            // FFmpeg row
            ui.horizontal(|ui| {
                ui.label(RichText::new("FFmpeg Decoder:").font(Theme::font_semibold(13.0)).color(Theme::TEXT));
                if state.status.ffmpeg.is_found() {
                    ui.label(RichText::new("● Active").font(Theme::font_semibold(12.0)).color(Color32::from_rgb(72, 199, 116)));
                } else {
                    ui.label(RichText::new("● Missing").font(Theme::font_semibold(12.0)).color(Color32::from_rgb(240, 80, 80)));
                }
            });
            ui.label(
                RichText::new(state.status.ffmpeg.display_label())
                    .font(Theme::font(11.5))
                    .color(Theme::TEXT_MUTED),
            );
            if let Some(ver) = &state.status.ffmpeg_version {
                ui.label(RichText::new(ver).font(Theme::font(10.5)).color(Theme::TEXT_FAINT));
            }

            ui.add_space(8.0);

            // FFprobe row
            ui.horizontal(|ui| {
                ui.label(RichText::new("FFprobe Inspector:").font(Theme::font_semibold(13.0)).color(Theme::TEXT));
                if state.status.ffprobe.is_found() {
                    ui.label(RichText::new("● Active").font(Theme::font_semibold(12.0)).color(Color32::from_rgb(72, 199, 116)));
                } else {
                    ui.label(RichText::new("● Missing").font(Theme::font_semibold(12.0)).color(Color32::from_rgb(240, 80, 80)));
                }
            });
            ui.label(
                RichText::new(state.status.ffprobe.display_label())
                    .font(Theme::font(11.5))
                    .color(Theme::TEXT_MUTED),
            );

            ui.add_space(14.0);

            // Downloader / Actions Section
            match &dl_state {
                DownloadState::Downloading(msg) => {
                    ui.label(RichText::new(msg).font(Theme::font_semibold(12.5)).color(Theme::ACCENT));
                    ui.add_space(4.0);
                    ui.add(ProgressBar::new(0.65).animate(true));
                    ui.add_space(4.0);
                    ui.label(RichText::new("Downloading official essentials build on the fly...").font(Theme::font(11.0)).color(Theme::TEXT_FAINT));
                }
                DownloadState::Extracting => {
                    ui.label(RichText::new("Extracting and verifying codecs...").font(Theme::font_semibold(12.5)).color(Theme::ACCENT));
                    ui.add_space(4.0);
                    ui.add(ProgressBar::new(0.9).animate(true));
                }
                DownloadState::Completed(msg) => {
                    ui.label(RichText::new(format!("✔ {msg}")).font(Theme::font_semibold(12.5)).color(Color32::from_rgb(72, 199, 116)));
                }
                DownloadState::Failed(err) => {
                    ui.label(RichText::new(format!("❌ Setup failed: {err}")).font(Theme::font_semibold(12.0)).color(Color32::from_rgb(240, 80, 80)));
                    ui.add_space(6.0);
                    if ui.add(primary_button("Retry Download")).clicked() {
                        state.downloader.start_download();
                    }
                }
                DownloadState::Idle => {
                    if !state.status.is_ready() {
                        ui.label(
                            RichText::new("Automatic Setup:")
                                .font(Theme::font_semibold(12.5))
                                .color(Theme::TEXT),
                        );
                        ui.label(
                            RichText::new("Click below to automatically download and unpack the minimal codecs on the fly into %LOCALAPPDATA%\\AirMedia\\bin.")
                                .font(Theme::font(11.5))
                                .color(Theme::TEXT_MUTED),
                        );
                        ui.add_space(8.0);

                        ui.horizontal(|ui| {
                            if ui.add(primary_button("⚡ Download Codecs On-the-Fly")).clicked() {
                                state.downloader.start_download();
                            }

                            if ui.add(secondary_button("Locate Folder...")).clicked() {
                                if let Some(folder) = crate::ui::FileDialog::new().pick_folder() {
                                    // Test if folder has ffmpeg
                                    let ff = folder.join("ffmpeg.exe");
                                    let fp = folder.join("ffprobe.exe");
                                    if ff.is_file() {
                                        // Copy to %LOCALAPPDATA%\AirMedia\bin
                                        if let Ok(local) = std::env::var("LOCALAPPDATA") {
                                            let dest = std::path::PathBuf::from(local).join("AirMedia").join("bin");
                                            let _ = std::fs::create_dir_all(&dest);
                                            let _ = std::fs::copy(&ff, dest.join("ffmpeg.exe"));
                                            if fp.is_file() {
                                                let _ = std::fs::copy(&fp, dest.join("ffprobe.exe"));
                                            }
                                        }
                                        state.status = check_dependencies();
                                    }
                                }
                            }
                        });
                    }
                }
            }

            ui.add_space(14.0);

            // Footer
            footer_row(ui, |ui| {
                if ui.add(secondary_button("Close")).clicked() {
                    *close = true;
                }
                if ui.add(secondary_button("Refresh Status")).clicked() {
                    state.status = check_dependencies();
                }
            });
        });
    }
}
