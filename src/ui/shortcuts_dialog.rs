use super::controls::section_label;
use super::dialog::show_dialog;
use super::theme::Theme;
use crate::shortcuts::{in_group, Group};
use eframe::egui::{self, RichText, Ui};

/// Keyboard shortcut help overlay (`?` / F1), generated from [`crate::shortcuts::SHORTCUTS`].
pub struct ShortcutsDialog;

impl ShortcutsDialog {
    pub fn show(ui: &mut Ui, is_open: &mut bool) {
        show_dialog(ui, "Keyboard shortcuts", is_open, 640.0, |ui, _close| {
            let max_h = ui.ctx().content_rect().height() * 0.75;
            egui::ScrollArea::vertical().max_height(max_h).show(ui, |ui| {
                ui.columns(2, |cols| {
                    // Left: playback and seeking; right: the rest.
                    for (col, groups) in cols.iter_mut().zip([
                        &[Group::Playback, Group::Seeking][..],
                        &[Group::AudioVideo, Group::Playlist, Group::Window][..],
                    ]) {
                        for group in groups {
                            section_label(col, group.name());
                            egui::Grid::new(("shortcuts", group.name()))
                                .num_columns(2)
                                .spacing([12.0, 6.0])
                                .show(col, |ui| {
                                    for s in in_group(*group) {
                                        ui.label(RichText::new(s.keys).font(Theme::font_semibold(12.5)).color(Theme::TEXT));
                                        ui.add(
                                            egui::Label::new(RichText::new(s.action).size(12.5).color(Theme::TEXT_MUTED))
                                                .wrap(),
                                        );
                                        ui.end_row();
                                    }
                                });
                            col.add_space(10.0);
                        }
                    }
                });
            });
        });
    }
}
