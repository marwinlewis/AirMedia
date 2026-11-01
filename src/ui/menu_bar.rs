use super::canvas::AspectRatioMode;
use super::controls::{trim_speed, SPEEDS};
use super::theme::Theme;
use crate::media::MediaPlayer;
use crate::playlist::Playlist;
use eframe::egui::{self, Response, RichText, Ui};
use std::path::PathBuf;

pub const MEDIA_EXTENSIONS: [&str; 14] = [
    "mp4", "mkv", "avi", "webm", "mov", "flv", "wmv", "mp3", "flac", "wav", "aac", "ogg", "opus", "m4a",
];

/// The app menu, shown as a popup from the top-left menu button. Each former menu-bar
/// category is a submenu.
pub struct MenuBarView;

pub struct MenuCtx<'a> {
    pub recent_files: &'a mut Vec<PathBuf>,
    pub aspect_mode: &'a mut AspectRatioMode,
    pub show_playlist: &'a mut bool,
    pub show_info: &'a mut bool,
    pub show_equalizer: &'a mut bool,
    pub show_stream: &'a mut bool,
    pub show_preferences: &'a mut bool,
    pub show_about: &'a mut bool,
    pub show_codecs: &'a mut bool,
    pub toggle_fullscreen: &'a mut bool,
    pub toast_msg: &'a mut Option<String>,
}

fn item(ui: &mut Ui, label: &str, shortcut: &str) -> Response {
    ui.add(
        egui::Button::new(label)
            .shortcut_text(RichText::new(shortcut).color(Theme::TEXT_FAINT).size(12.0))
            .frame_when_inactive(false),
    )
}

impl MenuBarView {
    pub fn show(ui: &mut Ui, player: &mut MediaPlayer, playlist: &mut Playlist, cx: MenuCtx<'_>) {
        ui.set_min_width(220.0);

        if item(ui, "Open file…", "Ctrl+O").clicked() {
            if let Some(path) = rfd::FileDialog::new()
                .add_filter("Media Files", &MEDIA_EXTENSIONS)
                .pick_file()
            {
                playlist.add(&path);
                playlist.current_index = Some(playlist.items.len() - 1);
                let _ = player.load_file(&path);
                if !cx.recent_files.contains(&path) {
                    cx.recent_files.push(path);
                }
            }
            ui.close();
        }

        if item(ui, "Open files…", "").clicked() {
            if let Some(files) = rfd::FileDialog::new()
                .add_filter("Media Files", &MEDIA_EXTENSIONS)
                .pick_files()
            {
                for f in files {
                    playlist.add(&f);
                    if !cx.recent_files.contains(&f) {
                        cx.recent_files.push(f);
                    }
                }
            }
            ui.close();
        }

        if item(ui, "Open folder…", "").clicked() {
            if let Some(folder) = rfd::FileDialog::new().pick_folder() {
                if let Ok(entries) = std::fs::read_dir(folder) {
                    for entry in entries.flatten() {
                        let p = entry.path();
                        if let Some(ext) = p.extension().and_then(|e| e.to_str()) {
                            if Playlist::is_supported_media_extension(ext) {
                                playlist.add(&p);
                            }
                        }
                    }
                }
            }
            ui.close();
        }

        if item(ui, "Open network stream…", "Ctrl+N").clicked() {
            *cx.show_stream = true;
            ui.close();
        }

        ui.menu_button("Recent", |ui| {
            if cx.recent_files.is_empty() {
                ui.label(RichText::new("Nothing here yet").color(Theme::TEXT_FAINT));
            } else {
                let mut to_load = None;
                for file in cx.recent_files.iter().rev() {
                    let label = file
                        .file_name()
                        .map(|f| f.to_string_lossy().to_string())
                        .unwrap_or_else(|| file.to_string_lossy().to_string());
                    if item(ui, &label, "").clicked() {
                        to_load = Some(file.clone());
                        ui.close();
                    }
                }
                if let Some(p) = to_load {
                    playlist.add(&p);
                    playlist.current_index = Some(playlist.items.len() - 1);
                    let _ = player.load_file(&p);
                }
            }
        });

        ui.separator();

        ui.menu_button("Playback", |ui| {
            let play_label = if player.is_playing { "Pause" } else { "Play" };
            if item(ui, play_label, "Space").clicked() {
                player.toggle_play();
                ui.close();
            }
            if item(ui, "Stop", "S").clicked() {
                player.stop();
                ui.close();
            }
            if item(ui, "Previous", "P").clicked() {
                if let Some(it) = playlist.prev() {
                    let _ = player.load_file(&it.path);
                }
                ui.close();
            }
            if item(ui, "Next", "N").clicked() {
                if let Some(it) = playlist.next() {
                    let _ = player.load_file(&it.path);
                }
                ui.close();
            }
            ui.separator();
            if item(ui, "Next frame", ".").clicked() {
                player.step_frame(true);
                ui.close();
            }
            if item(ui, "Previous frame", ",").clicked() {
                player.step_frame(false);
                ui.close();
            }
            ui.separator();
            ui.menu_button("Speed", |ui| {
                for s in SPEEDS {
                    let active = (player.playback_speed - s).abs() < 0.01;
                    if ui.selectable_label(active, format!("{}×", trim_speed(s))).clicked() {
                        player.set_speed(s);
                        ui.close();
                    }
                }
            });
            ui.menu_button("Jump", |ui| {
                for (label, delta, sc) in [
                    ("Forward 5 s", 5.0, "→"),
                    ("Forward 15 s", 15.0, "Shift+→"),
                    ("Forward 60 s", 60.0, "Ctrl+→"),
                    ("Back 5 s", -5.0, "←"),
                    ("Back 15 s", -15.0, "Shift+←"),
                    ("Back 60 s", -60.0, "Ctrl+←"),
                ] {
                    if item(ui, label, sc).clicked() {
                        let cur = player.current_time_secs();
                        player.seek(cur + delta);
                        ui.close();
                    }
                }
            });
        });

        ui.menu_button("Audio", |ui| {
            ui.menu_button("Track", |ui| {
                let mut chosen = None;
                if let Some(info) = &player.media_info {
                    for (idx, a) in info.audio_streams().iter().enumerate() {
                        let lang = a
                            .tags
                            .as_ref()
                            .and_then(|t| t.get("language"))
                            .and_then(|v| v.as_str())
                            .unwrap_or("und");
                        let codec = a.codec_name.as_deref().unwrap_or("audio");
                        let label = format!("Track {} · {} · {}", idx + 1, codec, lang);
                        if ui.selectable_label(player.active_audio_track == idx, label).clicked() {
                            chosen = Some(idx);
                            ui.close();
                        }
                    }
                }
                if let Some(idx) = chosen {
                    player.select_audio_track(idx);
                }
            });
            let mute_label = if player.is_muted() { "Unmute" } else { "Mute" };
            if item(ui, mute_label, "M").clicked() {
                player.toggle_mute();
                ui.close();
            }
            if item(ui, "Volume up", "↑").clicked() {
                player.set_volume((player.get_volume() + 0.05).min(1.5));
            }
            if item(ui, "Volume down", "↓").clicked() {
                player.set_volume((player.get_volume() - 0.05).max(0.0));
            }
            ui.separator();
            if item(ui, "Equalizer…", "Ctrl+E").clicked() {
                *cx.show_equalizer = true;
                ui.close();
            }
        });

        ui.menu_button("Video", |ui| {
            ui.menu_button("Aspect ratio", |ui| {
                for mode in AspectRatioMode::ALL {
                    if ui.selectable_label(*cx.aspect_mode == mode, mode.name()).clicked() {
                        *cx.aspect_mode = mode;
                        *cx.toast_msg = Some(format!("Aspect: {}", mode.name()));
                        ui.close();
                    }
                }
            });
            if item(ui, "Take snapshot", "Shift+S").clicked() {
                match player.take_snapshot() {
                    Ok(p) => *cx.toast_msg = Some(format!("Snapshot saved: {}", p.display())),
                    Err(e) => *cx.toast_msg = Some(format!("Snapshot failed: {e}")),
                }
                ui.close();
            }
            if item(ui, "Fullscreen", "F").clicked() {
                *cx.toggle_fullscreen = true;
                ui.close();
            }
        });

        ui.menu_button("Subtitles", |ui| {
            if item(ui, "Load file…", "").clicked() {
                if let Some(path) = rfd::FileDialog::new()
                    .add_filter("Subtitles", &["srt", "vtt", "ass"])
                    .pick_file()
                {
                    let _ = player.load_external_subtitle(path);
                }
                ui.close();
            }
            ui.separator();
            if ui.selectable_label(player.active_subtitle_track.is_none(), "Off").clicked() {
                player.select_subtitle_track(None);
                ui.close();
            }
            let mut chosen = None;
            for (idx, sub) in player.subtitles.iter().enumerate() {
                let label = format!("{} · {}", sub.name, sub.language);
                if ui.selectable_label(player.active_subtitle_track == Some(idx), label).clicked() {
                    chosen = Some(idx);
                    ui.close();
                }
            }
            if let Some(idx) = chosen {
                player.select_subtitle_track(Some(idx));
            }
        });

        ui.separator();

        let pl_label = if *cx.show_playlist { "Hide playlist" } else { "Show playlist" };
        if item(ui, pl_label, "L").clicked() {
            *cx.show_playlist = !*cx.show_playlist;
            ui.close();
        }
        if item(ui, "Media information", "I").clicked() {
            *cx.show_info = true;
            ui.close();
        }
        if item(ui, "Codec Information / Setup…", "").clicked() {
            *cx.show_codecs = true;
            ui.close();
        }
        if item(ui, "Preferences…", "Ctrl+P").clicked() {
            *cx.show_preferences = true;
            ui.close();
        }
        if item(ui, "About AirMedia", "").clicked() {
            *cx.show_about = true;
            ui.close();
        }

        ui.separator();

        if item(ui, "Quit", "Ctrl+Q").clicked() {
            ui.ctx().send_viewport_cmd(egui::ViewportCommand::Close);
        }
    }
}
