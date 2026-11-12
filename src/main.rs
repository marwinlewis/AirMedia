#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use airmedia::app::AirMediaApp;
use airmedia::app_info::APP_NAME;
use airmedia::media::is_url;
use airmedia::ui;
use eframe::egui;
use std::env;
use std::path::PathBuf;
use std::sync::Arc;

fn load_app_icon() -> egui::IconData {
    let size = 256u32;
    let rgba = ui::logo::render_unmultiplied_rgba(ui::logo::LogoVariant::Badge, size);
    egui::IconData {
        rgba,
        width: size,
        height: size,
    }
}

fn main() -> eframe::Result<()> {
    // Every argument is a file, folder, playlist or stream URL ("Open with" can pass many).
    // Flag-like arguments (e.g. macOS's -psn_*) are ignored.
    let cli_inputs: Vec<PathBuf> = env::args_os()
        .skip(1)
        .filter(|a| !a.to_string_lossy().starts_with('-'))
        .map(PathBuf::from)
        // Store absolute paths so the restored playlist works from any working directory.
        .map(|p| if is_url(&p) { p } else { std::path::absolute(&p).unwrap_or(p) })
        .collect();

    let viewport = egui::ViewportBuilder::default()
        .with_inner_size([1280.0, 780.0])
        .with_min_inner_size([480.0, 400.0])
        .with_title(APP_NAME)
        .with_decorations(false)
        .with_active(true)
        .with_drag_and_drop(true)
        .with_icon(Arc::new(load_app_icon()));

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
            app.open_paths(cli_inputs, true);

            Ok(Box::new(app))
        }),
    )
}
