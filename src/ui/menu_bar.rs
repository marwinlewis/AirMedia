use super::canvas::AspectRatioMode;
use super::controls::{trim_speed, SPEEDS};
use super::preferences_dialog::PreferencesState;
use super::theme::Theme;
use crate::ab_loop::AbLoop;
use crate::media::MediaPlayer;
use crate::playlist::Playlist;
use eframe::egui::{self, Response, RichText, Ui};
use std::path::PathBuf;

pub const MEDIA_EXTENSIONS: [&str; 26] = [
    "mp4", "mkv", "avi", "webm", "mov", "flv", "wmv", "ts", "m2ts", "mts", "m4v", "mpg", "mpeg", "ogv", "3gp",
    "mp3", "flac", "wav", "aac", "ogg", "opus", "m4a", "wma", "aiff", "mka", "wv",
];

pub const PLAYLIST_EXTENSIONS: [&str; 3] = ["m3u", "m3u8", "pls"];

/// Files/folders/URLs the user chose to open; the app expands and queues them.
pub struct OpenRequest {
    pub paths: Vec<PathBuf>,
    /// Start playing the first new item (otherwise only queue it).
    pub play: bool,
}

/// The app menu, shown as a popup from the top-left menu button. Each former menu-bar
/// category is a submenu.
pub struct MenuBarView;

pub struct MenuCtx<'a> {
    pub recent_files: &'a mut Vec<PathBuf>,
    pub open_request: &'a mut Option<OpenRequest>,
    pub aspect_mode: &'a mut AspectRatioMode,
    pub show_playlist: &'a mut bool,
    pub show_info: &'a mut bool,
    pub show_equalizer: &'a mut bool,
    pub show_stream: &'a mut bool,
    pub show_preferences: &'a mut bool,
    pub show_about: &'a mut bool,
    pub show_codecs: &'a mut bool,
    pub show_shortcuts: &'a mut bool,
    pub open_jump: &'a mut bool,
    pub ab_loop: &'a mut AbLoop,
    pub always_on_top: &'a mut bool,
    pub toggle_fullscreen: &'a mut bool,
    pub toast_msg: &'a mut Option<String>,
    pub prefs: &'a PreferencesState,
}

/// File picker for media, also accepting playlists.
pub fn media_dialog() -> crate::ui::FileDialog {
    let mut all: Vec<&str> = MEDIA_EXTENSIONS.to_vec();
    all.extend(PLAYLIST_EXTENSIONS);
    crate::ui::FileDialog::new()
        .add_filter("Media & playlists", &all)
        .add_filter("Media", &MEDIA_EXTENSIONS)
        .add_filter("Playlists", &PLAYLIST_EXTENSIONS)
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

        let idle = player.current_file.is_none();
        if item(ui, "Open file…", "Ctrl+O").clicked() {
            if let Some(path) = media_dialog().pick_file() {
                *cx.open_request = Some(OpenRequest { paths: vec![path], play: true });
            }
            ui.close();
        }

        if item(ui, "Open files…", "").clicked() {
            if let Some(files) = media_dialog().pick_files() {
                *cx.open_request = Some(OpenRequest { paths: files, play: idle });
            }
            ui.close();
        }

        if item(ui, "Open folder…", "").clicked() {
            if let Some(folder) = crate::ui::FileDialog::new().pick_folder() {
                *cx.open_request = Some(OpenRequest { paths: vec![folder], play: idle });
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
                for file in cx.recent_files.iter() {
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
                    *cx.open_request = Some(OpenRequest { paths: vec![p], play: true });
                }
                ui.separator();
                if item(ui, "Clear recent", "").clicked() {
                    cx.recent_files.clear();
                    ui.close();
                }
            }
        });

        ui.menu_button("Playlist", |ui| {
            if item(ui, "Import playlist…", "").clicked() {
                if let Some(path) = crate::ui::FileDialog::new()
                    .add_filter("Playlists", &PLAYLIST_EXTENSIONS)
                    .pick_file()
                {
                    *cx.open_request = Some(OpenRequest { paths: vec![path], play: idle });
                }
                ui.close();
            }
            if ui
                .add_enabled(!playlist.items.is_empty(), egui::Button::new("Save playlist…").frame_when_inactive(false))
                .clicked()
            {
                if let Some(path) = crate::ui::FileDialog::new()
                    .add_filter("M3U playlist", &["m3u8", "m3u"])
                    .set_file_name("playlist.m3u8")
                    .save_file()
                {
                    *cx.toast_msg = Some(match playlist.save_m3u(&path) {
                        Ok(()) => format!("Playlist saved ({} items)", playlist.items.len()),
                        Err(e) => e,
                    });
                }
                ui.close();
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
            if item(ui, "Jump to time…", "Ctrl+G").clicked() {
                *cx.open_jump = true;
                ui.close();
            }
            ui.menu_button("A-B loop", |ui| {
                let label = |p: Option<f64>| p.map_or_else(|| "–".to_string(), super::controls::format_time);
                if item(ui, &format!("Set A ({})", label(cx.ab_loop.a)), "B").clicked() {
                    let t = player.current_time_secs();
                    cx.ab_loop.set_a(t);
                    *cx.toast_msg = Some(format!("Loop A: {}", super::controls::format_time(t)));
                    ui.close();
                }
                if ui
                    .add_enabled(
                        cx.ab_loop.a.is_some(),
                        egui::Button::new(format!("Set B ({})", label(cx.ab_loop.b))).frame_when_inactive(false),
                    )
                    .clicked()
                {
                    let t = player.current_time_secs();
                    *cx.toast_msg = Some(if cx.ab_loop.set_b(t) {
                        format!("Loop B: {}", super::controls::format_time(t))
                    } else {
                        "Loop B must be after A".to_string()
                    });
                    ui.close();
                }
                if ui
                    .add_enabled(cx.ab_loop.a.is_some(), egui::Button::new("Clear loop").frame_when_inactive(false))
                    .clicked()
                {
                    cx.ab_loop.clear();
                    *cx.toast_msg = Some("A-B loop cleared".to_string());
                    ui.close();
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
            if item(ui, "Take screenshot", "Shift+S").clicked() {
                match player.take_screenshot(&cx.prefs.screenshot_dir, cx.prefs.screenshot_format) {
                    Ok(p) => *cx.toast_msg = Some(format!("Screenshot saved: {}", p.display())),
                    Err(e) => *cx.toast_msg = Some(format!("Screenshot failed: {e}")),
                }
                ui.close();
            }
            if item(ui, "Fullscreen", "F").clicked() {
                *cx.toggle_fullscreen = true;
                ui.close();
            }
            let on_top = *cx.always_on_top;
            if item(ui, if on_top { "✓ Always on top" } else { "Always on top" }, "Ctrl+T").clicked() {
                *cx.always_on_top = !on_top;
                *cx.toast_msg = Some(if on_top { "Always on top off" } else { "Always on top" }.to_string());
                ui.close();
            }
        });

        ui.menu_button("Subtitles", |ui| {
            if item(ui, "Load file…", "").clicked() {
                if let Some(path) = crate::ui::FileDialog::new()
                    .add_filter("Subtitles", &crate::media::subtitle::SUBTITLE_EXTENSIONS)
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
        if item(ui, "Keyboard shortcuts", "F1").clicked() {
            *cx.show_shortcuts = true;
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
