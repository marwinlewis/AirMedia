//! Rasterizes the master AirMedia SVG (`assets/airmedia.svg` / `dist/airmedia.svg`) on demand
//! at exact pixel sizes so the logo stays razor-sharp at any DPI and display size (from 16px
//! taskbar icons up to the full-bleed welcome backdrop).

use eframe::egui::{self, ColorImage, TextureHandle, TextureOptions};
use resvg::{tiny_skia, usvg};
use std::collections::HashMap;

pub const MASTER_SVG: &str = include_str!("../../assets/airmedia.svg");

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum LogoVariant {
    /// App icon and branding mark (transparent background).
    Badge,
    /// Transparent-background glowing emblem (welcome backdrop, adaptive icon foreground).
    Emblem,
}

/// Renders the AirMedia SVG at `size × size` physical pixels and returns premultiplied RGBA bytes
/// (matching `tiny_skia` and `egui::ColorImage::from_rgba_premultiplied`).
pub fn render_premultiplied_rgba(variant: LogoVariant, size: u32) -> Vec<u8> {
    render_pixmap(variant, size).take()
}

/// Renders the AirMedia SVG at `size × size` physical pixels and returns unpremultiplied RGBA bytes
/// (suitable for `egui::IconData` and PNG encoding).
pub fn render_unmultiplied_rgba(variant: LogoVariant, size: u32) -> Vec<u8> {
    let pixmap = render_pixmap(variant, size);
    let mut rgba = Vec::with_capacity((pixmap.width() * pixmap.height() * 4) as usize);
    for px in pixmap.pixels() {
        let c = px.demultiply();
        rgba.extend_from_slice(&[c.red(), c.green(), c.blue(), c.alpha()]);
    }
    rgba
}

fn render_pixmap(variant: LogoVariant, size: u32) -> tiny_skia::Pixmap {
    let size = size.max(1);
    let tree = parse_tree(variant);
    let svg_size = tree.size();
    let scale_x = size as f32 / svg_size.width();
    let scale_y = size as f32 / svg_size.height();
    let mut pixmap = tiny_skia::Pixmap::new(size, size).expect("non-zero pixmap");
    resvg::render(
        &tree,
        tiny_skia::Transform::from_scale(scale_x, scale_y),
        &mut pixmap.as_mut(),
    );
    pixmap
}

fn parse_tree(variant: LogoVariant) -> usvg::Tree {
    let svg_owned;
    let svg_src = match variant {
        LogoVariant::Badge => MASTER_SVG,
        LogoVariant::Emblem => {
            svg_owned = MASTER_SVG.replace(
                "<g id=\"background\">",
                "<g id=\"background\" display=\"none\">",
            );
            &svg_owned
        }
    };
    usvg::Tree::from_str(svg_src, &usvg::Options::default()).expect("Embedded airmedia.svg must be valid")
}

/// Caches crisp HiDPI GPU textures of the AirMedia logo keyed by `(variant, physical_px)`.
#[derive(Default)]
pub struct LogoCache {
    textures: HashMap<(LogoVariant, u32), TextureHandle>,
}

impl LogoCache {
    /// Returns a GPU texture rasterized at the exact physical pixel size for `logical_size` at the
    /// current `ctx.pixels_per_point()`, quantized slightly at large sizes to avoid thrashing
    /// during live window resizing.
    pub fn texture(&mut self, ctx: &egui::Context, variant: LogoVariant, logical_size: f32) -> TextureHandle {
        let raw_px = (logical_size * ctx.pixels_per_point()).round().clamp(16.0, 2048.0) as u32;
        let px = if raw_px <= 128 {
            raw_px
        } else {
            // Bucket large backdrop sizes to multiples of 32px so drag-resizing stays smooth.
            raw_px.div_ceil(32) * 32
        };

        self.textures
            .entry((variant, px))
            .or_insert_with(|| {
                let bytes = render_premultiplied_rgba(variant, px);
                let image = ColorImage::from_rgba_premultiplied([px as usize, px as usize], &bytes);
                let name = match variant {
                    LogoVariant::Badge => format!("airmedia_badge_{px}"),
                    LogoVariant::Emblem => format!("airmedia_emblem_{px}"),
                };
                ctx.load_texture(name, image, TextureOptions::LINEAR)
            })
            .clone()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn logo_renders_transparent_and_cleanly() {
        let emblem = render_unmultiplied_rgba(LogoVariant::Emblem, 512);
        assert_eq!(emblem.len(), 512 * 512 * 4);
        // Corners must be fully transparent
        assert_eq!(emblem[3], 0, "top-left corner must be transparent");
        assert_eq!(emblem[(511) * 4 + 3], 0, "top-right corner must be transparent");
        assert_eq!(emblem[(511 * 512) * 4 + 3], 0, "bottom-left corner must be transparent");
        assert_eq!(emblem[(511 * 512 + 511) * 4 + 3], 0, "bottom-right corner must be transparent");

        // Foreground logo must be substantial and opaque
        let opaque_count = emblem.chunks_exact(4).filter(|p| p[3] > 200).count();
        assert!(opaque_count > 20_000, "emblem must have substantial opaque content, found {opaque_count}");
    }
}
