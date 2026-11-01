#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use airmedia::app::AirMediaApp;
use airmedia::ui;
use eframe::egui;
use std::env;
use std::path::PathBuf;

fn main() -> eframe::Result<()> {
    let cli_file: Option<PathBuf> = env::args().nth(1).map(PathBuf::from);

    let native_options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_inner_size([1120.0, 680.0])
            .with_min_inner_size([560.0, 360.0])
            .with_title("AirMedia")
            .with_active(true)
            .with_drag_and_drop(true),
        ..Default::default()
    };

    eframe::run_native(
        "AirMedia",
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
