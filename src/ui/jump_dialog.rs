use super::controls::format_time;
use super::dialog::{footer_row, primary_button, secondary_button, show_dialog};
use super::theme::Theme;
use eframe::egui::{self, RichText, Ui};

/// Parses `ss`, `mm:ss` or `hh:mm:ss` (seconds may have a fraction). Minutes and seconds
/// after the first field must be below 60.
pub fn parse_clock(text: &str) -> Option<f64> {
    let parts: Vec<&str> = text.trim().split(':').map(str::trim).collect();
    if parts.is_empty() || parts.len() > 3 || parts.iter().any(|p| p.is_empty()) {
        return None;
    }
    let (secs_str, whole) = parts.split_last()?;
    let secs: f64 = secs_str.parse().ok().filter(|s: &f64| s.is_finite() && *s >= 0.0)?;
    let mut fields = Vec::with_capacity(whole.len());
    for p in whole {
        fields.push(p.parse::<u64>().ok()? as f64);
    }
    if !whole.is_empty() && secs >= 60.0 {
        return None;
    }
    match fields.as_slice() {
        [] => Some(secs),
        [m] => Some(m * 60.0 + secs),
        [h, m] if *m < 60.0 => Some(h * 3600.0 + m * 60.0 + secs),
        _ => None,
    }
}

pub struct JumpDialog;

impl JumpDialog {
    /// Returns the time to seek to when the user confirms a valid one.
    pub fn show(ui: &mut Ui, input: &mut String, is_open: &mut bool, duration: f64) -> Option<f64> {
        let mut result = None;
        show_dialog(ui, "Jump to time", is_open, 320.0, |ui, close| {
            ui.label(
                RichText::new(format!("Enter a time up to {}  (h:mm:ss, m:ss or seconds)", format_time(duration)))
                    .color(Theme::TEXT_MUTED),
            );
            ui.add_space(8.0);

            let text_resp = ui.add(
                egui::TextEdit::singleline(input)
                    .hint_text("1:23")
                    .desired_width(f32::INFINITY)
                    .margin(egui::Margin::symmetric(10, 8)),
            );
            if ui.memory(|m| m.focused().is_none()) {
                text_resp.request_focus();
            }

            let parsed = parse_clock(input).filter(|t| *t <= duration);
            ui.add_space(16.0);

            let mut go = text_resp.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter));
            footer_row(ui, |ui| {
                if ui.add_enabled(parsed.is_some(), primary_button("Jump")).clicked() {
                    go = true;
                }
                if ui.add(secondary_button("Cancel")).clicked() {
                    *close = true;
                }
            });

            if !input.trim().is_empty() && parsed.is_none() {
                ui.label(RichText::new("Not a valid time for this media").size(12.0).color(Theme::TEXT_FAINT));
            }
            if go {
                if let Some(t) = parsed {
                    result = Some(t);
                    *close = true;
                }
            }
        });
        result
    }
}
