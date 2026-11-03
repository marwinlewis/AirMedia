use super::icons::{icon_button, icon_button_ex, Icon};
use super::menu_bar::MEDIA_EXTENSIONS;
use super::theme::Theme;
use crate::media::MediaPlayer;
use crate::playlist::{Playlist, RepeatMode};
use eframe::egui::{self, Align2, Pos2, Rect, RichText, Sense, Ui, Vec2};

/// Slide-in playlist drawer docked to the right edge of the player.
pub struct PlaylistView;

impl PlaylistView {
    pub fn show(ui: &mut Ui, playlist: &mut Playlist, player: &mut MediaPlayer, is_open: &mut bool) {
        let frame = egui::Frame::NONE
            .fill(Theme::SURFACE)
            .inner_margin(egui::Margin::symmetric(14, 14));

        let close = egui::Panel::right("playlist_drawer")
            .frame(frame)
            .resizable(true)
            .drag_to_open(false)
            .show_separator_line(false)
            .default_size(320.0)
            .size_range(260.0..=520.0)
            .show_collapsible(ui, is_open, |ui| {
                Self::contents(ui, playlist, player)
            })
            .map(|r| r.inner)
            .unwrap_or(false);
        if close {
            *is_open = false;
        }
    }

    /// Returns true when the drawer asked to close.
    fn contents(ui: &mut Ui, playlist: &mut Playlist, player: &mut MediaPlayer) -> bool {
        let mut close = false;

        // Header
        ui.horizontal(|ui| {
            ui.label(RichText::new("Playlist").font(Theme::font_semibold(16.0)).color(Theme::TEXT));
            ui.label(RichText::new(playlist.items.len().to_string()).color(Theme::TEXT_FAINT));
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                if icon_button(ui, Icon::Close, 30.0).on_hover_text("Close (L)").clicked() {
                    close = true;
                }
            });
        });

        ui.add_space(8.0);

        // Search
        let search_frame = egui::Frame::NONE
            .fill(Theme::white(10))
            .corner_radius(Theme::radius_sm())
            .inner_margin(egui::Margin::symmetric(8, 2));
        search_frame.show(ui, |ui| {
            ui.horizontal(|ui| {
                let (r, _) = ui.allocate_exact_size(Vec2::splat(16.0), Sense::hover());
                Icon::Search.paint(ui.painter(), r, Theme::TEXT_FAINT);
                ui.add(
                    egui::TextEdit::singleline(&mut playlist.filter_query)
                        .hint_text("Search")
                        .frame(egui::Frame::NONE)
                        .desired_width(f32::INFINITY),
                );
            });
        });

        ui.add_space(6.0);

        // Toolbar
        ui.horizontal(|ui| {
            ui.spacing_mut().item_spacing.x = 2.0;
            if icon_button(ui, Icon::Plus, 30.0).on_hover_text("Add media").clicked() {
                if let Some(files) = crate::ui::FileDialog::new()
                    .add_filter("Media", &MEDIA_EXTENSIONS)
                    .pick_files()
                {
                    for f in files {
                        playlist.add(f);
                    }
                }
            }
            if icon_button_ex(ui, Icon::Shuffle, 30.0, playlist.is_shuffle)
                .on_hover_text(if playlist.is_shuffle { "Shuffle on" } else { "Shuffle off" })
                .clicked()
            {
                playlist.is_shuffle = !playlist.is_shuffle;
            }
            let repeat_on = playlist.repeat_mode != RepeatMode::Off;
            let repeat_resp = icon_button_ex(ui, Icon::Repeat, 30.0, repeat_on).on_hover_text(playlist.repeat_mode.name());
            if playlist.repeat_mode == RepeatMode::One {
                let badge = repeat_resp.rect.right_top() + Vec2::new(-7.0, 8.0);
                ui.painter().text(badge, Align2::CENTER_CENTER, "1", Theme::font_semibold(9.5), Theme::ACCENT_SOFT);
            }
            if repeat_resp.clicked() {
                playlist.repeat_mode = playlist.repeat_mode.cycle();
            }

            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                if !playlist.items.is_empty()
                    && icon_button(ui, Icon::Trash, 30.0).on_hover_text("Clear playlist").clicked()
                {
                    playlist.clear();
                }
            });
        });

        ui.add_space(6.0);

        let query = playlist.filter_query.to_lowercase();
        let mut play_index = None;
        let mut remove_index = None;

        egui::ScrollArea::vertical()
            .auto_shrink([false, false])
            .show(ui, |ui| {
                if playlist.items.is_empty() {
                    ui.add_space(48.0);
                    ui.vertical_centered(|ui| {
                        ui.label(RichText::new("Nothing queued").color(Theme::TEXT_MUTED));
                        ui.label(
                            RichText::new("Drop files here or press + to add media")
                                .color(Theme::TEXT_FAINT)
                                .size(12.0),
                        );
                    });
                    return;
                }

                ui.spacing_mut().item_spacing.y = 2.0;
                for (idx, item) in playlist.items.iter().enumerate() {
                    if !query.is_empty() && !item.title.to_lowercase().contains(&query) {
                        continue;
                    }
                    let is_current = playlist.current_index == Some(idx);
                    let (rect, resp) =
                        ui.allocate_exact_size(Vec2::new(ui.available_width(), 40.0), Sense::click());
                    let hovered = ui.rect_contains_pointer(rect);
                    let painter = ui.painter();

                    if is_current || hovered {
                        let fill = if is_current { Theme::white(14) } else { Theme::white(8) };
                        painter.rect_filled(rect, Theme::radius_sm(), fill);
                    }

                    // Leading: index number, or an "now playing" equalizer glyph.
                    let lead = Rect::from_min_size(rect.min, Vec2::new(36.0, rect.height()));
                    if is_current {
                        let levels = if player.is_playing { player.get_visualizer() } else { [0.3; 16] };
                        for (i, lvl) in [levels[1], levels[5], levels[9]].iter().enumerate() {
                            let h = (lvl.clamp(0.15, 1.0)) * 14.0;
                            let x = lead.center().x - 6.0 + i as f32 * 5.0;
                            let bar = Rect::from_min_max(
                                Pos2::new(x, lead.center().y + 7.0 - h),
                                Pos2::new(x + 3.0, lead.center().y + 7.0),
                            );
                            painter.rect_filled(bar, Theme::radius_full(), Theme::ACCENT_SOFT);
                        }
                    } else {
                        painter.text(
                            lead.center(),
                            Align2::CENTER_CENTER,
                            (idx + 1).to_string(),
                            Theme::font(12.0),
                            Theme::TEXT_FAINT,
                        );
                    }

                    // Title (truncated to fit, leaving room for the remove button)
                    let title_max = rect.width() - 36.0 - 34.0;
                    let color = if is_current { Theme::TEXT } else { Theme::TEXT_MUTED };
                    let font = if is_current { Theme::font_semibold(13.5) } else { Theme::font(13.5) };
                    let mut job = egui::text::LayoutJob::simple_singleline(item.title.clone(), font, color);
                    job.wrap = egui::text::TextWrapping::truncate_at_width(title_max);
                    let galley = painter.layout_job(job);
                    painter.galley(
                        Pos2::new(lead.max.x, rect.center().y - galley.size().y / 2.0),
                        galley,
                        color,
                    );

                    if hovered {
                        let x_rect = Rect::from_center_size(rect.right_center() - Vec2::new(18.0, 0.0), Vec2::splat(26.0));
                        let x_resp = ui.interact(x_rect, resp.id.with("remove"), Sense::click());
                        let c = if x_resp.hovered() { Theme::TEXT } else { Theme::TEXT_FAINT };
                        Icon::Close.paint(ui.painter(), x_rect.shrink(7.0), c);
                        if x_resp.clicked() {
                            remove_index = Some(idx);
                            continue;
                        }
                    }

                    if resp.double_clicked() || (resp.clicked() && !is_current) {
                        play_index = Some(idx);
                    }
                }
            });

        if let Some(idx) = play_index {
            playlist.current_index = Some(idx);
            if let Some(item) = playlist.items.get(idx) {
                let _ = player.load_file(&item.path);
            }
        }
        if let Some(idx) = remove_index {
            playlist.remove(idx);
        }

        close
    }
}
