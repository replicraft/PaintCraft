//! SVG sources are rasterized at display density and cached per egui context.
mod actions;
mod assets;
pub use assets::Icon;
use egui::{Color32, ColorImage, Context, Rect, TextureHandle, TextureOptions};
use std::{collections::VecDeque, sync::Arc};

const CACHE_BYTES: usize = 8 * 1024 * 1024;
const CACHE_ENTRIES: usize = 64;

#[derive(Default)]
struct Cache {
    entries: VecDeque<((Icon, u32), TextureHandle)>,
    bytes: usize,
}

fn rasterize(icon: Icon, size: u32) -> ColorImage {
    let tree = resvg::usvg::Tree::from_data(icon.svg(), &resvg::usvg::Options::default())
        .expect("bundled SVG must parse");
    let mut pixmap = resvg::tiny_skia::Pixmap::new(size, size).expect("bounded icon allocation");
    let transform = resvg::tiny_skia::Transform::from_scale(
        size as f32 / tree.size().width(),
        size as f32 / tree.size().height(),
    );
    resvg::render(&tree, transform, &mut pixmap.as_mut());
    ColorImage::from_rgba_premultiplied([size as usize; 2], pixmap.data())
}

fn physical_size(points: f32, ppp: f32) -> u32 {
    let required = (points * ppp).ceil().clamp(16.0, 256.0) as u32;
    [16, 24, 32, 48, 64, 96, 128, 192, 256]
        .into_iter()
        .find(|s| *s >= required)
        .unwrap_or(256)
}

fn texture(ctx: &Context, icon: Icon, size: u32) -> TextureHandle {
    let cache = ctx.data_mut(|d| {
        let id = egui::Id::unique("paintcraft-svg-icons");
        d.get_temp_mut_or_insert_with(id, || Arc::new(egui::mutex::Mutex::new(Cache::default())))
            .clone()
    });
    let mut cache = cache.lock();
    if let Some(index) = cache
        .entries
        .iter()
        .position(|(key, _)| *key == (icon, size))
    {
        let entry = cache.entries.remove(index).expect("cached index");
        let handle = entry.1.clone();
        cache.entries.push_back(entry);
        return handle;
    }
    let bytes = (size * size * 4) as usize;
    while cache.bytes + bytes > CACHE_BYTES || cache.entries.len() >= CACHE_ENTRIES {
        let Some(((.., old_size), _)) = cache.entries.pop_front() else {
            break;
        };
        cache.bytes -= (old_size * old_size * 4) as usize;
    }
    let handle = ctx.load_texture(
        format!("paintcraft-icon-{}-{size}", icon.name()),
        rasterize(icon, size),
        TextureOptions::LINEAR,
    );
    cache.bytes += bytes;
    cache.entries.push_back(((icon, size), handle.clone()));
    handle
}

pub fn image(ctx: &Context, icon: Icon, points: f32) -> egui::Image<'static> {
    let size = physical_size(points, ctx.pixels_per_point());
    let texture = texture(ctx, icon, size);
    egui::Image::new((texture.id(), egui::Vec2::splat(points)))
}

pub fn paint(painter: &egui::Painter, rect: Rect, icon: Icon) {
    let size = physical_size(
        rect.width().max(rect.height()),
        painter.ctx().pixels_per_point(),
    );
    let texture = texture(painter.ctx(), icon, size);
    painter.image(
        texture.id(),
        rect,
        Rect::from_min_max(egui::Pos2::ZERO, egui::pos2(1.0, 1.0)),
        Color32::WHITE,
    );
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_sheet_icon_is_standalone_vector_and_renders_with_transparency_at_multiple_sizes() {
        assert_eq!(Icon::ALL.len(), 88);
        let mut names = std::collections::HashSet::new();
        for icon in Icon::ALL {
            assert!(names.insert(icon.name()));
            let source = std::str::from_utf8(icon.svg()).unwrap();
            assert!(
                !source.contains("<image")
                    && !source.contains("href=")
                    && !source.contains("<text")
            );
            for size in [24, 64, 128] {
                let image = rasterize(icon, size);
                assert!(
                    image.pixels.iter().any(|p| p.a() > 0),
                    "{} at {size}",
                    icon.name()
                );
                assert!(image.pixels.iter().any(|p| p.a() == 0));
                assert_eq!(image.size, [size as usize; 2]);
            }
        }
    }

    #[test]
    fn icon_cache_enforces_entry_and_byte_limits_and_reuses_resident_textures() {
        for size in [24, 256] {
            let ctx = Context::default();
            for icon in Icon::ALL {
                let _ = texture(&ctx, icon, size);
            }
            let cache = ctx
                .data_mut(|d| {
                    d.get_temp::<Arc<egui::mutex::Mutex<Cache>>>(egui::Id::unique(
                        "paintcraft-svg-icons",
                    ))
                })
                .unwrap();
            {
                let cache = cache.lock();
                assert!(cache.entries.len() <= CACHE_ENTRIES);
                assert!(cache.bytes <= CACHE_BYTES);
                assert_eq!(cache.entries.back().unwrap().0, (Icon::ColorBalance, size));
            }
            let first = texture(&ctx, Icon::ColorBalance, size);
            let again = texture(&ctx, Icon::ColorBalance, size);
            assert_eq!(first.id(), again.id());
        }
    }

    #[test]
    fn density_selection_is_bounded_and_retains_high_dpi_detail() {
        assert_eq!(physical_size(22.0, 1.0), 24);
        assert_eq!(physical_size(22.0, 2.0), 48);
        assert_eq!(physical_size(22.0, 4.0), 96);
        assert_eq!(physical_size(22.0, 100.0), 256);
    }
}

#[cfg(test)]
mod review {
    use super::*;

    #[test]
    #[ignore = "writes an icon contact sheet to /tmp for visual review"]
    fn render_all_icons_for_review() {
        let mut sheet = resvg::tiny_skia::Pixmap::new(12 * 88, 6 * 88).unwrap();
        sheet.fill(resvg::tiny_skia::Color::from_rgba8(242, 244, 248, 255));
        for (index, icon) in Icon::ALL.into_iter().enumerate() {
            let tree =
                resvg::usvg::Tree::from_data(icon.svg(), &resvg::usvg::Options::default()).unwrap();
            let transform = resvg::tiny_skia::Transform::from_translate(
                (index % 12) as f32 * 88.0 + 12.0,
                (index / 12) as f32 * 88.0 + 12.0,
            );
            resvg::render(&tree, transform, &mut sheet.as_mut());
        }
        sheet.save_png("/tmp/paintcraft-svg-icons.png").unwrap();
    }
}
