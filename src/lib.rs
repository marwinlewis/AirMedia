pub mod app;
pub mod app_info;
pub mod media;
pub mod playlist;
pub mod ui;

#[cfg(target_os = "android")]
use android_activity::AndroidApp;

#[cfg(target_os = "android")]
#[no_mangle]
fn android_main(app: AndroidApp) {
    let mut native_options = eframe::NativeOptions::default();
    native_options.android_app = Some(app);

    let _ = eframe::run_native(
        "AirMedia",
        native_options,
        Box::new(|cc| {
            ui::theme::Theme::install(&cc.egui_ctx);
            let app = app::AirMediaApp::new();
            Ok(Box::new(app))
        }),
    );
}
