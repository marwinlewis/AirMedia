//! The home screen: top navigation over a full-bleed hero banner, followed by horizontally
//! scrolling rows of media cards. Videos and Music are grids; search filters everything.

use super::artwork::{paint_cover, ArtworkCache};
use super::controls::{menu_row, option_row};
use super::icons::{lerp_color, pill_button, primary_circle_button, Icon, PillStyle};
use super::theme::Theme;
use crate::library::{FeaturedReason, Library, LibraryEntry, MediaKind, format_duration_short};
use crate::persistence::SessionState;
use eframe::egui::{
    self, text::LayoutJob, text::TextWrapping, Align2, Color32, CornerRadius, FontId, Id, Popup, PopupCloseBehavior, Pos2, Rect,
    Response, RichText, Sense, Stroke, StrokeKind, TextureHandle, Ui, UiBuilder, Vec2,
};
use std::path::{Path, PathBuf};
use std::sync::Arc;

pub const NAV_HEIGHT: f32 = 68.0;
/// Height of the text under a card.
const CAPTION_H: f32 = 46.0;
const CARD_GAP: f32 = 10.0;
/// Ideal card width; the actual width fills the row with whole cards.
const CARD_TARGET_W: f32 = 268.0;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum BrowseTab {
    #[default]
    Home,
    Videos,
    Music,
}

#[derive(Default)]
pub struct BrowseState {
    pub tab: BrowseTab,
    pub search_open: bool,
    pub search_query: String,
    pub focus_search: bool,
    scroll_y: f32,
}

impl BrowseState {
    /// Opens the search field and gives it the keyboard.
    pub fn open_search(&mut self) {
        self.search_open = true;
        self.focus_search = true;
    }

    pub fn close_search(&mut self) {
        self.search_open = false;
        self.search_query.clear();
    }
}

/// Things the home screen asks the app to do once the frame is painted.
#[derive(Debug, Clone, PartialEq)]
pub enum BrowseAction {
    Play(PathBuf),
    PlayFromStart(PathBuf),
    ShowDetails(PathBuf),
    AddToPlaylist(PathBuf),
    RemoveFromPlaylist(PathBuf),
    ForgetHistory(PathBuf),
    OpenFile,
    OpenFiles,
    OpenFolder,
    OpenStream,
    TogglePlaylist,
}

pub struct BrowseCx<'a> {
    pub library: &'a Library,
    pub art: &'a mut ArtworkCache,
    pub session: &'a SessionState,
    pub now_playing: Option<&'a Path>,
    pub is_playing: bool,
    pub up_next: Option<&'a Path>,
    pub playlist_len: usize,
    pub playlist_open: bool,
    /// The current video frame, so the banner can show what is really playing.
    pub live_frame: Option<&'a TextureHandle>,
    pub logo: &'a TextureHandle,
    pub levels: [f32; 16],
    pub actions: &'a mut Vec<BrowseAction>,
}

/// Paints the whole screen into `rect`; returns the app-menu button so the app can attach its menu.
pub fn show(ui: &mut Ui, rect: Rect, state: &mut BrowseState, cx: &mut BrowseCx<'_>) -> Response {
    let mut page = ui.new_child(UiBuilder::new().max_rect(rect).id_salt("browse_page"));
    page.shrink_clip_rect(rect);
    page.spacing_mut().item_spacing = Vec2::ZERO;

    let searching = !state.search_query.trim().is_empty();
    let scroll_id = if searching { "browse_search" } else {
        match state.tab {
            BrowseTab::Home => "browse_home",
            BrowseTab::Videos => "browse_videos",
            BrowseTab::Music => "browse_music",
        }
    };
    let view_h = rect.height();
    let output = egui::ScrollArea::vertical()
        .id_salt(scroll_id)
        .auto_shrink([false, false])
        .show(&mut page, |ui| {
            ui.spacing_mut().item_spacing = Vec2::ZERO;
            if searching {
                let hits = cx.library.search(&state.search_query);
                let heading = match hits.len() {
                    0 => format!("No results for \u{201c}{}\u{201d}", state.search_query.trim()),
                    1 => "1 result".to_string(),
                    n => format!("{n} results"),
                };
                grid_page(ui, &heading, &hits, "Try a different title or folder name.", cx);
            } else {
                match state.tab {
                    BrowseTab::Home if cx.library.is_empty() => welcome(ui, view_h, cx),
                    BrowseTab::Home => home_page(ui, view_h, cx),
                    BrowseTab::Videos => {
                        let items = cx.library.videos.clone();
                        grid_page(ui, "Videos", &items, "Videos you open or add to your playlist appear here.", cx);
                    }
                    BrowseTab::Music => {
                        let items = cx.library.music.clone();
                        grid_page(ui, "Music", &items, "Music you open or add to your playlist appears here.", cx);
                    }
                }
            }
        });
    state.scroll_y = output.state.clamped_offset().y;

    navigation(ui, rect, state, cx)
}

fn page_padding(width: f32) -> f32 {
    (width * 0.04).clamp(16.0, 56.0)
}

fn single_line(painter: &egui::Painter, text: &str, font: FontId, color: Color32, max_w: f32) -> Arc<egui::Galley> {
    let mut job = LayoutJob::simple_singleline(text.to_owned(), font, color);
    job.wrap = TextWrapping::truncate_at_width(max_w);
    painter.layout_job(job)
}

// ---------------------------------------------------------------------------------------------
// Navigation
// ---------------------------------------------------------------------------------------------

fn navigation(ui: &mut Ui, rect: Rect, state: &mut BrowseState, cx: &mut BrowseCx<'_>) -> Response {
    let nav = Rect::from_min_size(rect.min, Vec2::new(rect.width(), NAV_HEIGHT));
    let painter = ui.painter_at(nav);

    // Transparent over the banner, solid once content scrolls under it.
    let solid = (state.scroll_y / 140.0).clamp(0.0, 1.0);
    let always_solid = !matches!(state.tab, BrowseTab::Home) || !state.search_query.trim().is_empty();
    let solid = if always_solid { 1.0 } else { solid };
    Theme::paint_vertical_gradient(&painter, nav, Theme::black(190), Theme::black(0));
    painter.rect_filled(nav, CornerRadius::ZERO, Theme::BG.gamma_multiply(solid));

    // Swallow clicks so cards scrolled under the bar can't be hit through it.
    let _ = ui.interact(nav, Id::unique("browse_nav_block"), Sense::click());

    let pad = page_padding(rect.width());
    let compact = rect.width() < 760.0;
    let cy = nav.center().y;

    // Brand
    let logo_rect = Rect::from_center_size(Pos2::new(nav.min.x + pad + 18.0, cy), Vec2::splat(36.0));
    let tex = egui::load::SizedTexture::from_handle(cx.logo);
    egui::Image::new(tex)
        .corner_radius(CornerRadius::same(8))
        .paint_at(ui, logo_rect);
    let mut x = logo_rect.max.x + 10.0;
    if !compact {
        let wordmark = painter.layout_no_wrap("AirMedia".to_owned(), Theme::font_bold(21.0), Theme::TEXT);
        painter.galley(Pos2::new(x, cy - wordmark.size().y / 2.0), wordmark.clone(), Theme::TEXT);
        x += wordmark.size().x;
    }
    let brand = Rect::from_min_max(Pos2::new(logo_rect.min.x, nav.min.y), Pos2::new(x, nav.max.y));
    let brand_resp = ui
        .interact(brand, Id::unique("browse_brand"), Sense::click())
        .on_hover_cursor(egui::CursorIcon::PointingHand);
    if brand_resp.clicked() {
        state.tab = BrowseTab::Home;
        state.close_search();
    }
    x += if compact { 8.0 } else { 26.0 };

    // Tabs
    for (tab, label) in [(BrowseTab::Home, "Home"), (BrowseTab::Videos, "Videos"), (BrowseTab::Music, "Music")] {
        let active = state.tab == tab && state.search_query.trim().is_empty();
        let font = if active { Theme::font_semibold(14.5) } else { Theme::font(14.5) };
        let galley = painter.layout_no_wrap(label.to_owned(), font, Color32::PLACEHOLDER);
        let tab_rect = Rect::from_min_size(Pos2::new(x, cy - 18.0), Vec2::new(galley.size().x + 20.0, 36.0));
        let resp = ui
            .interact(tab_rect, Id::unique(("browse_tab", label)), Sense::click())
            .on_hover_cursor(egui::CursorIcon::PointingHand);
        let color = if active || resp.hovered() { Theme::TEXT } else { Theme::TEXT_MUTED };
        painter.galley(tab_rect.center() - galley.size() / 2.0, galley.clone(), color);
        if active {
            let w = galley.size().x;
            let bar = Rect::from_center_size(Pos2::new(tab_rect.center().x, tab_rect.max.y - 2.0), Vec2::new(w, 2.5));
            painter.rect_filled(bar, Theme::radius_full(), Theme::ACCENT);
        }
        if resp.clicked() {
            state.tab = tab;
            state.close_search();
        }
        x += tab_rect.width();
    }

    // Right-hand tools, laid out from the edge inward.
    let tools_min_x = x + 8.0;
    let tools = Rect::from_min_max(Pos2::new(tools_min_x, nav.min.y), Pos2::new(nav.max.x - pad + 6.0, nav.max.y));
    let mut tools_ui = ui.new_child(
        UiBuilder::new()
            .id_salt("browse_tools")
            .max_rect(tools)
            .layout(egui::Layout::right_to_left(egui::Align::Center)),
    );
    tools_ui.spacing_mut().item_spacing = Vec2::new(2.0, 0.0);

    let menu_resp = super::controls::icon_button_big(&mut tools_ui, Icon::Menu, 38.0).on_hover_text("Menu");

    let playlist_resp = super::controls::icon_button_ex_big(&mut tools_ui, Icon::Playlist, 38.0, cx.playlist_open)
        .on_hover_text("Playlist (L)");
    if cx.playlist_len > 0 {
        let label = if cx.playlist_len > 99 { "99+".to_string() } else { cx.playlist_len.to_string() };
        let g = tools_ui.painter().layout_no_wrap(label, Theme::font_semibold(9.5), Theme::TEXT);
        let badge_w = (g.size().x + 8.0).max(15.0);
        let badge = Rect::from_center_size(playlist_resp.rect.right_top() + Vec2::new(-8.0, 9.0), Vec2::new(badge_w, 15.0));
        tools_ui.painter().rect_filled(badge, Theme::radius_full(), Theme::ACCENT);
        tools_ui.painter().galley(badge.center() - g.size() / 2.0, g, Theme::TEXT);
    }
    if playlist_resp.clicked() {
        cx.actions.push(BrowseAction::TogglePlaylist);
    }

    let open_resp = if compact {
        super::controls::icon_button_big(&mut tools_ui, Icon::Plus, 38.0).on_hover_text("Open media")
    } else {
        tools_ui.add_space(6.0);
        let r = pill_button(&mut tools_ui, Some(Icon::Plus), "Open", PillStyle::Secondary, 34.0).on_hover_text("Open media");
        tools_ui.add_space(6.0);
        r
    };
    Popup::menu(&open_resp)
        .close_behavior(PopupCloseBehavior::CloseOnClick)
        .gap(6.0)
        .width(230.0)
        .show(|ui| {
            for (icon, label, shortcut, action) in [
                (Icon::Film, "Open file\u{2026}", "O", BrowseAction::OpenFile),
                (Icon::Plus, "Add files\u{2026}", "", BrowseAction::OpenFiles),
                (Icon::Folder, "Add folder\u{2026}", "", BrowseAction::OpenFolder),
                (Icon::Globe, "Network stream\u{2026}", "Ctrl+N", BrowseAction::OpenStream),
            ] {
                if menu_row(ui, icon, label, shortcut).clicked() {
                    cx.actions.push(action);
                }
            }
        });

    // Search: an icon that expands into a field.
    if state.search_open {
        let field_w = (tools_ui.available_width() - 8.0).clamp(90.0, 260.0);
        let (field, _) = tools_ui.allocate_exact_size(Vec2::new(field_w, 34.0), Sense::hover());
        let fp = tools_ui.painter();
        fp.rect(field, Theme::radius_sm(), Theme::black(170), Stroke::new(1.0, Theme::white(110)), StrokeKind::Inside);
        let icon_r = Rect::from_center_size(field.left_center() + Vec2::new(16.0, 0.0), Vec2::splat(16.0));
        Icon::Search.paint(fp, icon_r, Theme::TEXT_MUTED);
        let edit_rect = Rect::from_min_max(field.min + Vec2::new(30.0, 7.0), field.max - Vec2::new(28.0, 6.0));
        let edit = tools_ui.put(
            edit_rect,
            egui::TextEdit::singleline(&mut state.search_query)
                .id(Id::unique("browse_search_field"))
                .hint_text("Titles, folders")
                .frame(egui::Frame::NONE)
                .desired_width(edit_rect.width()),
        );
        if state.focus_search {
            edit.request_focus();
            state.focus_search = false;
        }
        let x_rect = Rect::from_center_size(field.right_center() - Vec2::new(14.0, 0.0), Vec2::splat(22.0));
        let x_resp = tools_ui.interact(x_rect, Id::unique("browse_search_clear"), Sense::click()).on_hover_text("Close search");
        let c = if x_resp.hovered() { Theme::TEXT } else { Theme::TEXT_MUTED };
        Icon::Close.paint(tools_ui.painter(), x_rect.shrink(5.0), c);
        if x_resp.clicked() {
            state.close_search();
        }
    } else if super::controls::icon_button_big(&mut tools_ui, Icon::Search, 38.0).on_hover_text("Search (Ctrl+F)").clicked() {
        state.open_search();
    }

    menu_resp
}

// ---------------------------------------------------------------------------------------------
// Home page
// ---------------------------------------------------------------------------------------------

fn home_page(ui: &mut Ui, view_h: f32, cx: &mut BrowseCx<'_>) {
    let width = ui.available_width();
    hero(ui, width, view_h, cx);
    let library = cx.library;
    for row in &library.rows {
        media_row(ui, width, &row.title, &row.items, cx);
    }
    ui.add_space(28.0);
}

fn hero(ui: &mut Ui, width: f32, view_h: f32, cx: &mut BrowseCx<'_>) {
    let library = cx.library;
    let Some((idx, reason)) = library.featured(cx.now_playing, cx.up_next) else { return };
    let entry = &library.entries[idx];
    let path = entry.path.as_path();

    let height = (view_h * 0.66).clamp(320.0, 620.0).min(width * 0.75 + 120.0);
    let (rect, _) = ui.allocate_exact_size(Vec2::new(width, height), Sense::hover());
    let painter = ui.painter_at(rect);
    let square = CornerRadius::ZERO;

    let live = if reason == FeaturedReason::NowPlaying { cx.live_frame } else { None };
    match live {
        Some(tex) => paint_cover(&painter, rect, square, tex, Color32::WHITE),
        None => cx.art.paint(&painter, rect, square, path, &entry.title, entry.kind),
    }

    // Scrims: text side on the left, dissolve into the page at the bottom.
    let side = Rect::from_min_size(rect.min, Vec2::new((width * 0.72).max(320.0), height));
    Theme::paint_horizontal_gradient(&painter, side, Theme::BG.gamma_multiply(0.92), Theme::BG.gamma_multiply(0.0));
    let foot = Rect::from_min_max(Pos2::new(rect.min.x, rect.max.y - height * 0.55), rect.max);
    Theme::paint_vertical_gradient(&painter, foot, Theme::BG.gamma_multiply(0.0), Theme::BG);
    if width < 720.0 {
        // Text spans the full width here, past where the side scrim has faded out.
        painter.rect_filled(rect, square, Theme::BG.gamma_multiply(0.4));
    }

    let pad = page_padding(width);
    let content_w = (width - pad * 2.0).min((width * 0.58).max(420.0)).min(720.0);
    let meta = cx.art.meta(path).cloned();

    let title = meta
        .as_ref()
        .filter(|m| m.audio_only)
        .and_then(|m| m.title.clone())
        .unwrap_or_else(|| entry.title.clone());

    let title_size = (width * 0.042).clamp(28.0, 52.0);
    let mut job = LayoutJob::simple(title, Theme::font_bold(title_size), Theme::TEXT, content_w);
    job.wrap = TextWrapping { max_rows: 2, max_width: content_w, overflow_character: Some('\u{2026}'), break_anywhere: false };
    let title_galley = painter.layout_job(job);

    let eyebrow = match reason {
        FeaturedReason::NowPlaying if cx.is_playing => "NOW PLAYING",
        FeaturedReason::NowPlaying => "PAUSED",
        FeaturedReason::Continue if entry.kind == MediaKind::Audio => "CONTINUE LISTENING",
        FeaturedReason::Continue => "CONTINUE WATCHING",
        FeaturedReason::UpNext => "UP NEXT",
        FeaturedReason::Recent => "RECENTLY PLAYED",
    };

    let mut details: Vec<String> = Vec::new();
    if let Some(artist) = meta.as_ref().filter(|m| m.audio_only).and_then(|m| m.artist.clone()) {
        details.push(artist);
    }
    if let Some(m) = &meta {
        let s = m.summary();
        if !s.is_empty() {
            details.push(s);
        }
    }
    if let Some(f) = entry.format_label() {
        details.push(f);
    } else {
        details.push(entry.kind.label().to_string());
    }
    let detail_line = details.join("  \u{00b7}  ");

    // Stacked upward from the bottom edge of the banner.
    let button_h = if width < 560.0 { 40.0 } else { 46.0 };
    let buttons_y = rect.max.y - 34.0 - button_h;
    let progress = progress_of(cx.session, path, cx.art.meta(path).map(|m| m.duration_secs));
    let mut y = buttons_y;
    if let Some((frac, left)) = progress {
        y -= 30.0;
        let bar = Rect::from_min_size(Pos2::new(rect.min.x + pad, y + 8.0), Vec2::new(content_w.min(280.0), 4.0));
        painter.rect_filled(bar, Theme::radius_full(), Theme::white(70));
        let fill = Rect::from_min_size(bar.min, Vec2::new(bar.width() * frac, bar.height()));
        painter.rect_filled(fill, Theme::radius_full(), Theme::ACCENT);
        painter.text(
            bar.right_center() + Vec2::new(12.0, 0.0),
            Align2::LEFT_CENTER,
            format!("{} left", format_duration_short(left)),
            Theme::font(13.0),
            Theme::TEXT_MUTED,
        );
    }
    y -= 34.0;
    let detail_galley = single_line(&painter, &detail_line, Theme::font(14.5), Theme::TEXT_MUTED, content_w);
    painter.galley(Pos2::new(rect.min.x + pad, y + 4.0), detail_galley, Theme::TEXT_MUTED);
    y -= title_galley.size().y + 4.0;
    painter.galley(Pos2::new(rect.min.x + pad, y), title_galley, Theme::TEXT);
    y -= 26.0;

    let eyebrow_color = Theme::ACCENT_SOFT;
    let mut eyebrow_x = rect.min.x + pad;
    if reason == FeaturedReason::NowPlaying && cx.is_playing {
        eyebrow_x += equalizer_glyph(&painter, Pos2::new(eyebrow_x, y + 15.0), 14.0, &cx.levels, eyebrow_color) + 8.0;
    }
    painter.text(Pos2::new(eyebrow_x, y + 9.0), Align2::LEFT_CENTER, eyebrow, Theme::font_bold(12.5), eyebrow_color);

    // Buttons
    let row = Rect::from_min_size(Pos2::new(rect.min.x + pad, buttons_y), Vec2::new(width - pad * 2.0, button_h));
    let mut buttons = ui.new_child(UiBuilder::new().id_salt("hero_buttons").max_rect(row).layout(egui::Layout::left_to_right(egui::Align::Center)));
    buttons.spacing_mut().item_spacing = Vec2::new(10.0, 0.0);
    let play_label = match reason {
        FeaturedReason::NowPlaying => "Open Player",
        FeaturedReason::Continue => "Resume",
        _ => "Play",
    };
    if pill_button(&mut buttons, Some(Icon::Play), play_label, PillStyle::Primary, button_h).clicked() {
        cx.actions.push(BrowseAction::Play(entry.path.clone()));
    }
    if pill_button(&mut buttons, Some(Icon::Info), "More Info", PillStyle::Secondary, button_h).clicked() {
        cx.actions.push(BrowseAction::ShowDetails(entry.path.clone()));
    }
}

/// First-run screen: nothing fake, just the brand and the ways to get media in.
fn welcome(ui: &mut Ui, view_h: f32, cx: &mut BrowseCx<'_>) {
    let width = ui.available_width();
    let (rect, _) = ui.allocate_exact_size(Vec2::new(width, view_h.max(420.0)), Sense::hover());
    let painter = ui.painter_at(rect);
    painter.rect_filled(rect, CornerRadius::ZERO, Theme::BG);

    let pad = page_padding(width);

    // The flame, large and dim, as the backdrop.
    if width > 680.0 {
        let size = (rect.height() * 1.15).min(width * 0.8);
        let backdrop = Rect::from_center_size(Pos2::new(rect.max.x - size * 0.36, rect.center().y), Vec2::splat(size));
        paint_cover(&painter, backdrop, CornerRadius::ZERO, cx.logo, Color32::from_gray(210));
        let left = Rect::from_min_max(backdrop.min, Pos2::new(backdrop.center().x, backdrop.max.y));
        Theme::paint_horizontal_gradient(&painter, left, Theme::BG, Theme::BG.gamma_multiply(0.0));
        let top = Rect::from_min_max(backdrop.min, Pos2::new(backdrop.max.x, backdrop.min.y + size * 0.3));
        Theme::paint_vertical_gradient(&painter, top, Theme::BG, Theme::BG.gamma_multiply(0.0));
        let bottom = Rect::from_min_max(Pos2::new(backdrop.min.x, backdrop.max.y - size * 0.3), backdrop.max);
        Theme::paint_vertical_gradient(&painter, bottom, Theme::BG.gamma_multiply(0.0), Theme::BG);
    }

    let content_w = (width - pad * 2.0).min(560.0);
    let title_size = (width * 0.05).clamp(30.0, 54.0);
    let mut job = LayoutJob::simple("Everything you want to watch, in one place.".to_owned(), Theme::font_bold(title_size), Theme::TEXT, content_w);
    job.wrap.max_width = content_w;
    let title = painter.layout_job(job);
    let body = painter.layout(
        "Drop files anywhere in this window, or open a file, a whole folder or a network stream. AirMedia builds your library as you go.".to_owned(),
        Theme::font(16.0),
        Theme::TEXT_MUTED,
        content_w,
    );

    let block_h = title.size().y + 14.0 + body.size().y + 26.0 + 46.0 + 30.0 + 22.0;
    let mut y = rect.center().y - block_h / 2.0 + 20.0;
    let x = rect.min.x + pad;
    let title_h = title.size().y;
    painter.galley(Pos2::new(x, y), title, Theme::TEXT);
    y += title_h + 14.0;
    let body_h = body.size().y;
    painter.galley(Pos2::new(x, y), body, Theme::TEXT_MUTED);
    y += body_h + 26.0;

    let row = Rect::from_min_size(Pos2::new(x, y), Vec2::new(width - pad * 2.0, 46.0));
    let mut buttons = ui.new_child(UiBuilder::new().id_salt("welcome_buttons").max_rect(row).layout(egui::Layout::left_to_right(egui::Align::Center)));
    buttons.spacing_mut().item_spacing = Vec2::new(10.0, 0.0);
    if pill_button(&mut buttons, Some(Icon::Play), "Open File", PillStyle::Primary, 46.0).clicked() {
        cx.actions.push(BrowseAction::OpenFile);
    }
    if pill_button(&mut buttons, Some(Icon::Folder), "Add Folder", PillStyle::Secondary, 46.0).clicked() {
        cx.actions.push(BrowseAction::OpenFolder);
    }
    if width > 480.0 && pill_button(&mut buttons, Some(Icon::Globe), "Stream", PillStyle::Secondary, 46.0).clicked() {
        cx.actions.push(BrowseAction::OpenStream);
    }
    y += 46.0 + 30.0;

    // Keyboard hints from the shortcut table.
    let mut hint_x = x;
    for (keys, label) in crate::shortcuts::hints() {
        let key = painter.layout_no_wrap(keys.to_owned(), Theme::font_semibold(12.0), Theme::TEXT);
        let key_rect = Rect::from_min_size(Pos2::new(hint_x, y), key.size() + Vec2::new(12.0, 6.0));
        let text = painter.layout_no_wrap(label.to_owned(), Theme::font(12.5), Theme::TEXT_FAINT);
        if key_rect.max.x + text.size().x + 8.0 > rect.max.x - pad {
            break;
        }
        painter.rect_filled(key_rect, Theme::radius_sm(), Theme::white(22));
        painter.galley(key_rect.center() - key.size() / 2.0, key, Theme::TEXT);
        let text_w = text.size().x;
        painter.galley(Pos2::new(key_rect.max.x + 7.0, key_rect.center().y - text.size().y / 2.0), text, Theme::TEXT_FAINT);
        hint_x = key_rect.max.x + 7.0 + text_w + 18.0;
    }
}

/// Fraction watched and seconds left, when `path` has a saved position.
fn progress_of(session: &SessionState, path: &Path, known_duration: Option<f64>) -> Option<(f32, f64)> {
    let (pos, saved_dur) = session.resume_progress(path)?;
    let dur = if saved_dur > 0.0 { saved_dur } else { known_duration.unwrap_or(0.0) };
    (dur > 0.0).then(|| (((pos / dur) as f32).clamp(0.0, 1.0), (dur - pos).max(0.0)))
}

/// Three little bars; returns the width drawn.
fn equalizer_glyph(painter: &egui::Painter, baseline_left: Pos2, height: f32, levels: &[f32; 16], color: Color32) -> f32 {
    let w = height / 4.0;
    for (i, lvl) in [levels[1], levels[5], levels[9]].iter().enumerate() {
        let h = lvl.clamp(0.2, 1.0) * height;
        let x = baseline_left.x + i as f32 * (w + 2.0);
        let bar = Rect::from_min_max(Pos2::new(x, baseline_left.y - h), Pos2::new(x + w, baseline_left.y));
        painter.rect_filled(bar, Theme::radius_full(), color);
    }
    3.0 * w + 4.0
}

// ---------------------------------------------------------------------------------------------
// Rows and grids
// ---------------------------------------------------------------------------------------------

fn media_row(ui: &mut Ui, width: f32, title: &str, items: &[usize], cx: &mut BrowseCx<'_>) {
    if items.is_empty() {
        return;
    }
    let pad = page_padding(width);
    let usable = width - pad * 2.0;
    let per_page = ((usable + CARD_GAP) / (CARD_TARGET_W + CARD_GAP)).floor().max(1.0);
    let card_w = if per_page < 2.0 {
        // Phones: leave the next card peeking in, so it's obvious the row scrolls.
        usable * 0.8
    } else {
        (usable - CARD_GAP * (per_page - 1.0)) / per_page
    };
    let art_h = card_w * 9.0 / 16.0;
    let title_h = 40.0;
    let total_h = title_h + art_h + CAPTION_H + 22.0;

    let (rect, _) = ui.allocate_exact_size(Vec2::new(width, total_h), Sense::hover());
    if !ui.is_rect_visible(rect) {
        return;
    }

    let id = Id::unique(("media_row", title));
    let heading = single_line(ui.painter(), title, Theme::font_bold(19.0), Theme::TEXT, usable);
    ui.painter().galley(Pos2::new(rect.min.x + pad, rect.min.y + 8.0), heading, Theme::TEXT);

    let strip = Rect::from_min_size(rect.min + Vec2::new(0.0, title_h), Vec2::new(width, art_h + CAPTION_H));
    let mut row = ui.new_child(UiBuilder::new().id_salt(id).max_rect(strip));
    // A little vertical room for the hover zoom; cards never spill into the gutters' neighbours.
    row.shrink_clip_rect(strip.expand2(Vec2::new(0.0, 10.0)));

    let content_w = items.len() as f32 * (card_w + CARD_GAP) - CARD_GAP + pad * 2.0;
    let max_offset = (content_w - width).max(0.0);
    let offset_id = id.with("offset");
    let mut target = row.ctx().data(|d| d.get_temp::<f32>(offset_id)).unwrap_or(0.0).clamp(0.0, max_offset);

    // Touch and mouse drags pan sideways and hand the vertical part to the page.
    let drag = row.interact(strip, id.with("drag"), Sense::drag());
    if drag.dragged() {
        let delta = drag.drag_delta();
        target = (target - delta.x).clamp(0.0, max_offset);
        row.scroll_with_delta(Vec2::new(0.0, delta.y));
    }
    let hovered = row.rect_contains_pointer(strip);
    if hovered {
        let wheel_x = row.ctx().input(|i| i.smooth_scroll_delta.x);
        if wheel_x != 0.0 {
            target = (target - wheel_x).clamp(0.0, max_offset);
        }
    }

    let offset = if drag.dragged() {
        row.ctx().animate_value_with_time(id.with("anim"), target, 0.0)
    } else {
        row.ctx().animate_value_with_time(id.with("anim"), target, 0.28)
    };

    for (i, item) in items.iter().enumerate() {
        let x = rect.min.x + pad + i as f32 * (card_w + CARD_GAP) - offset;
        if x + card_w < strip.min.x - 20.0 || x > strip.max.x + 20.0 {
            continue;
        }
        let card_rect = Rect::from_min_size(Pos2::new(x, strip.min.y), Vec2::new(card_w, art_h + CAPTION_H));
        media_card(&mut row, card_rect, art_h, *item, id.with(*item), cx);
    }

    // Page buttons in the gutters, Netflix-style, shown when the row is hovered.
    let gutter = pad.max(38.0);
    let page = per_page.max(1.0) * (card_w + CARD_GAP);
    let art_band = Rect::from_min_size(strip.min, Vec2::new(width, art_h));
    if hovered && offset > 1.0 {
        let r = Rect::from_min_size(art_band.min, Vec2::new(gutter, art_h));
        if row_chevron(&mut row, r, Icon::ChevronLeft, true, id.with("prev")).clicked() {
            target = (target - page).max(0.0);
        }
    }
    if hovered && offset < max_offset - 1.0 {
        let r = Rect::from_min_max(Pos2::new(art_band.max.x - gutter, art_band.min.y), art_band.max);
        if row_chevron(&mut row, r, Icon::ChevronRight, false, id.with("next")).clicked() {
            target = (target + page).min(max_offset);
        }
    }
    row.ctx().data_mut(|d| d.insert_temp(offset_id, target));
}

fn row_chevron(ui: &mut Ui, rect: Rect, icon: Icon, left: bool, id: Id) -> Response {
    let resp = ui.interact(rect, id, Sense::click()).on_hover_cursor(egui::CursorIcon::PointingHand);
    let strength = if resp.hovered() { 215 } else { 150 };
    let painter = ui.painter_at(rect);
    let (a, b) = if left { (Theme::black(strength), Theme::black(0)) } else { (Theme::black(0), Theme::black(strength)) };
    Theme::paint_horizontal_gradient(&painter, rect, a, b);
    let size = if resp.hovered() { 30.0 } else { 26.0 };
    icon.paint(&painter, Rect::from_center_size(rect.center(), Vec2::splat(size)), Theme::TEXT);
    resp
}

/// A page with a heading and a responsive grid; only cards on screen are built, so a library
/// with thousands of items stays smooth.
fn grid_page(ui: &mut Ui, heading: &str, items: &[usize], empty_note: &str, cx: &mut BrowseCx<'_>) {
    let width = ui.available_width();
    let pad = page_padding(width);
    let usable = width - pad * 2.0;
    ui.add_space(NAV_HEIGHT + 14.0);

    let (head_rect, _) = ui.allocate_exact_size(Vec2::new(width, 52.0), Sense::hover());
    let head = single_line(ui.painter(), heading, Theme::font_bold(28.0), Theme::TEXT, usable);
    ui.painter().galley(Pos2::new(head_rect.min.x + pad, head_rect.min.y + 4.0), head, Theme::TEXT);

    if items.is_empty() {
        let (rect, _) = ui.allocate_exact_size(Vec2::new(width, 120.0), Sense::hover());
        ui.painter().text(rect.left_top() + Vec2::new(pad, 2.0), Align2::LEFT_TOP, empty_note, Theme::font(14.5), Theme::TEXT_MUTED);
        let row = Rect::from_min_size(rect.left_top() + Vec2::new(pad, 36.0), Vec2::new(usable, 42.0));
        let mut buttons = ui.new_child(UiBuilder::new().id_salt("empty_buttons").max_rect(row).layout(egui::Layout::left_to_right(egui::Align::Center)));
        if pill_button(&mut buttons, Some(Icon::Plus), "Open Media", PillStyle::Primary, 42.0).clicked() {
            cx.actions.push(BrowseAction::OpenFiles);
        }
        return;
    }

    let cols = ((usable + CARD_GAP) / (CARD_TARGET_W * 0.82 + CARD_GAP)).floor().max(1.0);
    let card_w = (usable - CARD_GAP * (cols - 1.0)) / cols;
    let art_h = card_w * 9.0 / 16.0;
    let cell_h = art_h + CAPTION_H + 14.0;
    let rows = (items.len() as f32 / cols).ceil();
    let (grid, _) = ui.allocate_exact_size(Vec2::new(width, rows * cell_h + 32.0), Sense::hover());

    let clip = ui.clip_rect();
    let id = Id::unique(("media_grid", heading));
    let first_row = (((clip.min.y - grid.min.y) / cell_h).floor() - 1.0).max(0.0) as usize;
    let last_row = (((clip.max.y - grid.min.y) / cell_h).ceil() + 1.0).max(0.0) as usize;
    let cols_n = cols as usize;
    for row in first_row..=last_row {
        for col in 0..cols_n {
            let Some(item) = items.get(row * cols_n + col) else { continue };
            let pos = grid.min + Vec2::new(pad + col as f32 * (card_w + CARD_GAP), row as f32 * cell_h);
            let rect = Rect::from_min_size(pos, Vec2::new(card_w, art_h + CAPTION_H));
            media_card(ui, rect, art_h, *item, id.with(*item), cx);
        }
    }
}

fn card_subtitle(entry: &LibraryEntry, cx: &BrowseCx<'_>, progress: Option<(f32, f64)>, is_current: bool) -> String {
    if is_current {
        return if cx.is_playing { "Now playing".into() } else { "Paused".into() };
    }
    if let Some((_, left)) = progress {
        return format!("{} left", format_duration_short(left));
    }
    let mut parts = Vec::new();
    if let Some(meta) = cx.art.meta(&entry.path) {
        if meta.audio_only {
            if let Some(artist) = &meta.artist {
                parts.push(artist.clone());
            }
        }
        let s = meta.summary();
        if !s.is_empty() {
            parts.push(s);
        }
    }
    if parts.is_empty() {
        parts.push(entry.kind.label().to_string());
    }
    if let Some(f) = entry.format_label() {
        parts.push(f);
    }
    parts.join(" \u{00b7} ")
}

fn media_card(ui: &mut Ui, rect: Rect, art_h: f32, item: usize, id: Id, cx: &mut BrowseCx<'_>) {
    let library = cx.library;
    let entry = &library.entries[item];
    let path = entry.path.as_path();

    let resp = ui.interact(rect, id, Sense::click()).on_hover_cursor(egui::CursorIcon::PointingHand);
    let art_base = Rect::from_min_size(rect.min, Vec2::new(rect.width(), art_h));
    let info_rect = Rect::from_center_size(art_base.right_top() + Vec2::new(-22.0, 22.0), Vec2::splat(30.0));
    let info = ui.interact(info_rect, id.with("info"), Sense::click()).on_hover_text("More info");

    let hot = resp.hovered() || info.hovered() || resp.has_focus();
    let t = ui.ctx().animate_bool_with_time(id.with("hover"), hot, 0.14);
    let art_rect = art_base.expand2(Vec2::new(rect.width(), art_h) * 0.022 * t);

    if ui.is_rect_visible(rect.expand(8.0)) {
        let radius = Theme::radius_md();
        let is_current = cx.now_playing == Some(path);
        let duration = cx.art.meta(path).map(|m| m.duration_secs);
        let progress = progress_of(cx.session, path, duration);
        let painter = ui.painter().clone();

        if t > 0.0 {
            painter.rect_filled(art_rect.translate(Vec2::new(0.0, 6.0)).expand(2.0), Theme::radius_lg(), Theme::black((90.0 * t) as u8));
        }
        cx.art.paint(&painter, art_rect, radius, path, &entry.title, entry.kind);
        cx.art.paint_scrim(&painter, art_rect, radius, 110);

        if entry.kind == MediaKind::Stream {
            let live = painter.layout_no_wrap("STREAM".to_owned(), Theme::font_bold(10.0), Theme::TEXT);
            let tag = Rect::from_min_size(art_rect.left_bottom() + Vec2::new(8.0, -24.0), live.size() + Vec2::new(10.0, 5.0));
            painter.rect_filled(tag, Theme::radius_sm(), Theme::black(170));
            painter.galley(tag.center() - live.size() / 2.0, live, Theme::TEXT);
        }

        // Watched fraction
        if let Some((frac, _)) = progress {
            let bar = Rect::from_min_max(art_rect.left_bottom() + Vec2::new(8.0, -9.0), art_rect.right_bottom() + Vec2::new(-8.0, -5.0));
            painter.rect_filled(bar, Theme::radius_full(), Theme::white(70));
            let fill = Rect::from_min_size(bar.min, Vec2::new(bar.width() * frac, bar.height()));
            painter.rect_filled(fill, Theme::radius_full(), Theme::ACCENT);
        }

        if is_current {
            painter.rect_stroke(art_rect, radius, Stroke::new(2.0, Theme::ACCENT), StrokeKind::Inside);
            if cx.is_playing {
                let base = art_rect.left_top() + Vec2::new(10.0, 24.0);
                let bg = Rect::from_min_size(art_rect.left_top() + Vec2::new(6.0, 6.0), Vec2::new(30.0, 24.0));
                painter.rect_filled(bg, Theme::radius_sm(), Theme::black(170));
                equalizer_glyph(&painter, base + Vec2::new(1.0, 2.0), 14.0, &cx.levels, Theme::ACCENT_SOFT);
            }
        }

        if t > 0.0 {
            painter.rect_stroke(
                art_rect,
                radius,
                Stroke::new(2.0, Color32::from_white_alpha((230.0 * t) as u8)),
                StrokeKind::Inside,
            );
            // Hover affordances: play in the middle, info in the corner.
            let circle = 46.0_f32.min(art_h * 0.42) * (0.85 + 0.15 * t);
            painter.circle_filled(art_rect.center(), circle / 2.0, Color32::from_white_alpha((245.0 * t) as u8));
            let play = Rect::from_center_size(art_rect.center() + Vec2::new(1.5, 0.0), Vec2::splat(circle * 0.46));
            Icon::Play.paint(&painter, play, Theme::BG.gamma_multiply(t));

            let fill = lerp_color(Theme::black(170), Theme::white(60), if info.hovered() { 1.0 } else { 0.0 }).gamma_multiply(t);
            painter.circle_filled(info_rect.center(), 15.0, fill);
            Icon::Info.paint(&painter, info_rect.shrink(7.0), Color32::from_white_alpha((255.0 * t) as u8));
        }

        // Caption
        let max_w = rect.width() - 2.0;
        let title_color = if hot { Theme::TEXT } else { Color32::from_rgb(232, 232, 238) };
        let title = single_line(&painter, &entry.title, Theme::font_semibold(14.0), title_color, max_w);
        let truncated = title.elided;
        painter.galley(Pos2::new(rect.min.x + 1.0, art_base.max.y + 7.0), title, title_color);
        let sub_text = card_subtitle(entry, cx, progress, is_current);
        let sub_color = if is_current { Theme::ACCENT_SOFT } else { Theme::TEXT_FAINT };
        let sub = single_line(&painter, &sub_text, Theme::font(12.5), sub_color, max_w);
        painter.galley(Pos2::new(rect.min.x + 1.0, art_base.max.y + 26.0), sub, sub_color);

        if truncated {
            let tip_text = entry.title.clone();
            resp.clone().on_hover_text_at_pointer(tip_text);
        }
    }

    if info.clicked() {
        cx.actions.push(BrowseAction::ShowDetails(entry.path.clone()));
    } else if resp.clicked() {
        cx.actions.push(BrowseAction::Play(entry.path.clone()));
    }
    resp.context_menu(|ui| {
        ui.set_max_width(230.0);
        if menu_row(ui, Icon::Play, "Play", "").clicked() {
            cx.actions.push(BrowseAction::Play(entry.path.clone()));
            ui.close();
        }
        if menu_row(ui, Icon::Info, "More info", "").clicked() {
            cx.actions.push(BrowseAction::ShowDetails(entry.path.clone()));
            ui.close();
        }
        ui.separator();
        if entry.in_playlist {
            if menu_row(ui, Icon::Close, "Remove from playlist", "").clicked() {
                cx.actions.push(BrowseAction::RemoveFromPlaylist(entry.path.clone()));
                ui.close();
            }
        } else if menu_row(ui, Icon::Plus, "Add to playlist", "").clicked() {
            cx.actions.push(BrowseAction::AddToPlaylist(entry.path.clone()));
            ui.close();
        }
        if entry.in_history && menu_row(ui, Icon::Trash, "Remove from history", "").clicked() {
            cx.actions.push(BrowseAction::ForgetHistory(entry.path.clone()));
            ui.close();
        }
    });
}

/// Details overlay for one library item.
pub fn details_modal(ctx: &egui::Context, entry: &LibraryEntry, cx: &mut BrowseCx<'_>) -> bool {
    let screen = ctx.content_rect();
    let width = (screen.width() - 32.0).clamp(280.0, 640.0);
    let path = entry.path.as_path();
    let mut close = false;

    let frame = egui::Frame::NONE
        .fill(Theme::SURFACE)
        .corner_radius(Theme::radius_xl())
        .stroke(Stroke::new(1.0, Theme::BORDER));
    let modal = egui::Modal::new(Id::unique("details_modal"))
        .frame(frame)
        .backdrop_color(Theme::black(190))
        .show(ctx, |ui| {
            ui.set_width(width);
            ui.spacing_mut().item_spacing = Vec2::ZERO;

            let art_h = width * 9.0 / 16.0;
            let (art_rect, _) = ui.allocate_exact_size(Vec2::new(width, art_h), Sense::hover());
            let radius = CornerRadius { nw: 12, ne: 12, sw: 0, se: 0 };
            let painter = ui.painter_at(art_rect);
            cx.art.paint(&painter, art_rect, radius, path, &entry.title, entry.kind);
            let foot = Rect::from_min_max(Pos2::new(art_rect.min.x, art_rect.max.y - art_h * 0.7), art_rect.max);
            Theme::paint_vertical_gradient(&painter, foot, Theme::SURFACE.gamma_multiply(0.0), Theme::SURFACE);

            let close_rect = Rect::from_center_size(art_rect.right_top() + Vec2::new(-26.0, 26.0), Vec2::splat(34.0));
            let close_resp = ui.interact(close_rect, Id::unique("details_close"), Sense::click()).on_hover_text("Close (Esc)");
            let fill = if close_resp.hovered() { Theme::black(230) } else { Theme::black(170) };
            painter.circle_filled(close_rect.center(), 17.0, fill);
            Icon::Close.paint(&painter, close_rect.shrink(10.0), Theme::TEXT);
            if close_resp.clicked() {
                close = true;
            }

            let meta = cx.art.meta(path).cloned();
            let title = meta
                .as_ref()
                .filter(|m| m.audio_only)
                .and_then(|m| m.title.clone())
                .unwrap_or_else(|| entry.title.clone());
            let inner = width - 56.0;
            let mut job = LayoutJob::simple(title, Theme::font_bold(30.0), Theme::TEXT, inner);
            job.wrap = TextWrapping { max_rows: 2, max_width: inner, overflow_character: Some('\u{2026}'), break_anywhere: false };
            let title = painter.layout_job(job);
            let title_pos = Pos2::new(art_rect.min.x + 28.0, art_rect.max.y - title.size().y - 62.0);
            painter.galley(title_pos, title, Theme::TEXT);

            let progress = progress_of(cx.session, path, meta.as_ref().map(|m| m.duration_secs));
            let is_current = cx.now_playing == Some(path);

            let row = Rect::from_min_size(art_rect.left_bottom() + Vec2::new(28.0, -52.0), Vec2::new(inner, 42.0));
            let mut buttons = ui.new_child(UiBuilder::new().id_salt("details_buttons").max_rect(row).layout(egui::Layout::left_to_right(egui::Align::Center)));
            buttons.spacing_mut().item_spacing = Vec2::new(8.0, 0.0);
            let play_label = if is_current {
                "Open Player"
            } else if progress.is_some() {
                "Resume"
            } else {
                "Play"
            };
            if pill_button(&mut buttons, Some(Icon::Play), play_label, PillStyle::Primary, 42.0).clicked() {
                cx.actions.push(BrowseAction::Play(entry.path.clone()));
                close = true;
            }
            if progress.is_some() && pill_button(&mut buttons, Some(Icon::Repeat), "Start Over", PillStyle::Secondary, 42.0).clicked() {
                cx.actions.push(BrowseAction::PlayFromStart(entry.path.clone()));
                close = true;
            }
            if entry.in_playlist {
                if primary_circle_button(&mut buttons, Icon::Check, 42.0, Theme::SURFACE_HI, Theme::TEXT)
                    .on_hover_text("In your playlist. Click to remove.")
                    .clicked()
                {
                    cx.actions.push(BrowseAction::RemoveFromPlaylist(entry.path.clone()));
                }
            } else if primary_circle_button(&mut buttons, Icon::Plus, 42.0, Theme::SURFACE_HI, Theme::TEXT)
                .on_hover_text("Add to playlist")
                .clicked()
            {
                cx.actions.push(BrowseAction::AddToPlaylist(entry.path.clone()));
            }

            // Facts
            ui.add_space(6.0);
            egui::Frame::NONE
                .inner_margin(egui::Margin { left: 28, right: 28, top: 8, bottom: 24 })
                .show(ui, |ui| {
                    ui.spacing_mut().item_spacing = Vec2::new(0.0, 6.0);
                    if let Some((frac, left)) = progress {
                        let (bar_rect, _) = ui.allocate_exact_size(Vec2::new(inner.min(300.0), 4.0), Sense::hover());
                        ui.painter().rect_filled(bar_rect, Theme::radius_full(), Theme::white(50));
                        let fill = Rect::from_min_size(bar_rect.min, Vec2::new(bar_rect.width() * frac, 4.0));
                        ui.painter().rect_filled(fill, Theme::radius_full(), Theme::ACCENT);
                        ui.label(RichText::new(format!("{} left", format_duration_short(left))).size(13.0).color(Theme::TEXT_MUTED));
                        ui.add_space(4.0);
                    }
                    let mut facts: Vec<String> = Vec::new();
                    if let Some(m) = &meta {
                        if let Some(artist) = m.artist.as_ref().filter(|_| m.audio_only) {
                            facts.push(artist.clone());
                        }
                        let s = m.summary();
                        if !s.is_empty() {
                            facts.push(s);
                        }
                        if let Some(c) = &m.video_codec {
                            facts.push(c.to_uppercase());
                        }
                    }
                    if let Some(f) = entry.format_label() {
                        facts.push(f);
                    } else {
                        facts.push(entry.kind.label().to_string());
                    }
                    ui.label(RichText::new(facts.join("  \u{00b7}  ")).size(14.5).color(Theme::TEXT));
                    ui.add_space(6.0);
                    let location = entry.path.to_string_lossy();
                    ui.add(egui::Label::new(RichText::new(location.as_ref()).size(12.5).color(Theme::TEXT_FAINT)).wrap());
                    if entry.in_history {
                        ui.add_space(8.0);
                        if option_row(ui, false, "Remove from history").clicked() {
                            cx.actions.push(BrowseAction::ForgetHistory(entry.path.clone()));
                            close = true;
                        }
                    }
                });
        });

    close || modal.should_close()
}
