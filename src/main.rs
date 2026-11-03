#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use airmedia::app::AirMediaApp;
use airmedia::app_info::APP_NAME;
use airmedia::ui;
use eframe::egui;
use std::env;
use std::path::PathBuf;
use std::sync::Arc;

fn load_app_icon() -> Option<egui::IconData> {
    let icon_bytes = include_bytes!("../assets/icon.png");
    let img = image::load_from_memory(icon_bytes).ok()?;
    let rgba = img.to_rgba8();
    let (width, height) = rgba.dimensions();
    Some(egui::IconData {
        rgba: rgba.into_raw(),
        width,
        height,
    })
}

fn main() -> eframe::Result<()> {
    let cli_file: Option<PathBuf> = env::args().nth(1).map(PathBuf::from);

    let mut viewport = egui::ViewportBuilder::default()
        .with_inner_size([1120.0, 680.0])
        .with_min_inner_size([560.0, 360.0])
        .with_title(APP_NAME)
        .with_active(true)
        .with_drag_and_drop(true);

    if let Some(icon) = load_app_icon() {
        viewport = viewport.with_icon(Arc::new(icon));
    }

    let native_options = eframe::NativeOptions {
        viewport,
        ..Default::default()
    };

    eframe::run_native(
        APP_NAME,
        native_options,
        Box::new(move |cc| {
            ui::theme::Theme::install(&cc.egui_ctx);

            let mut app = AirMediaApp::new();

            if let Some(path) = cli_file {
                if path.exists() {
                    app.playlist.add(&path);
                    app.playlist.current_index = Some(0);
                    let _ = app.player.load_file(&path);
                }
            }

            Ok(Box::new(app))
        }),
    )
}
