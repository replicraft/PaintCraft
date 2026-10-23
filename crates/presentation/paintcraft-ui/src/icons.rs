//! SVG sources are rasterized at display density and cached per egui context.
mod actions;
mod assets;
mod classic_image;
mod classic_menus;
mod classic_shapes;
pub use assets::Icon;
use egui::{Color32, ColorImage, Context, Rect, TextureHandle, TextureOptions};
use std::{collections::VecDeque, sync::Arc};

const CACHE_BYTES: usize = 8 * 1024 * 1024;
const CACHE_ENTRIES: usize = 64;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Variant {
    Sheet,
    ClassicShape,
    ClassicMenu,
    ClassicImageLight,
    ClassicImageDark,
}

#[derive(Default)]
struct Cache {
    entries: VecDeque<((Icon, u32, Variant), TextureHandle)>,
    bytes: usize,
}

fn rasterize(icon: Icon, size: u32) -> ColorImage {
    rasterize_variant(icon, size, Variant::Sheet)
}

fn rasterize_variant(icon: Icon, size: u32, variant: Variant) -> ColorImage {
    let source = match variant {
        Variant::Sheet => icon.svg(),
        Variant::ClassicShape => {
            classic_shapes::source(icon).expect("Classic shape must have an outline SVG")
        }
        Variant::ClassicMenu => {
            classic_menus::source(icon).expect("Classic menu icon must have an outline SVG")
        }
        Variant::ClassicImageLight | Variant::ClassicImageDark => {
            classic_image::source(icon, variant == Variant::ClassicImageDark)
                .expect("Classic Image control must have theme artwork")
        }
    };
    let tree = resvg::usvg::Tree::from_data(source, &resvg::usvg::Options::default())
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
    variant_texture(ctx, icon, size, Variant::Sheet)
}

fn variant_texture(ctx: &Context, icon: Icon, size: u32, variant: Variant) -> TextureHandle {
    let cache = ctx.data_mut(|d| {
        let id = egui::Id::unique("paintcraft-svg-icons");
        d.get_temp_mut_or_insert_with(id, || Arc::new(egui::mutex::Mutex::new(Cache::default())))
            .clone()
    });
    let mut cache = cache.lock();
    if let Some(index) = cache
        .entries
        .iter()
        .position(|(key, _)| *key == (icon, size, variant))
    {
        let entry = cache.entries.remove(index).expect("cached index");
        let handle = entry.1.clone();
        cache.entries.push_back(entry);
        return handle;
    }
    let bytes = (size * size * 4) as usize;
    while cache.bytes + bytes > CACHE_BYTES || cache.entries.len() >= CACHE_ENTRIES {
        let Some(((_, old_size, _), _)) = cache.entries.pop_front() else {
            break;
        };
        cache.bytes -= (old_size * old_size * 4) as usize;
    }
    let handle = ctx.load_texture(
        format!("paintcraft-icon-{}-{variant:?}-{size}", icon.name()),
        match variant {
            Variant::Sheet => rasterize(icon, size),
            _ => rasterize_variant(icon, size, variant),
        },
        TextureOptions::LINEAR,
    );
    cache.bytes += bytes;
    cache
        .entries
        .push_back(((icon, size, variant), handle.clone()));
    handle
}

pub fn image(ctx: &Context, icon: Icon, points: f32) -> egui::Image<'static> {
    let size = physical_size(points, ctx.pixels_per_point());
    let texture = texture(ctx, icon, size);
    egui::Image::new((texture.id(), egui::Vec2::splat(points)))
}

/// Tintable original outline SVGs for the 23 Classic shape-gallery choices.
/// The texture cache separates these sources from the colored sheet variants.
pub fn classic_shape_image(
    ctx: &Context,
    icon: Icon,
    points: f32,
    ink: Color32,
) -> egui::Image<'static> {
    let size = physical_size(points, ctx.pixels_per_point());
    let texture = variant_texture(ctx, icon, size, Variant::ClassicShape);
    egui::Image::new((texture.id(), egui::Vec2::splat(points))).tint(ink)
}

/// Theme-tinted menu variants; unmapped operations retain their default artwork.
pub fn classic_menu_image(
    ctx: &Context,
    icon: Icon,
    points: f32,
    ink: Color32,
) -> egui::Image<'static> {
    if classic_menus::source(icon).is_none() {
        return image(ctx, icon, points);
    }
    let size = physical_size(points, ctx.pixels_per_point());
    let texture = variant_texture(ctx, icon, size, Variant::ClassicMenu);
    egui::Image::new((texture.id(), egui::Vec2::splat(points))).tint(ink)
}

/// Image ribbon/transform art preserves blue accents in light and dark themes.
pub fn classic_image(ctx: &Context, icon: Icon, points: f32, dark: bool) -> egui::Image<'static> {
    if classic_image::source(icon, dark).is_none() {
        return image(ctx, icon, points);
    }
    let variant = if dark {
        Variant::ClassicImageDark
    } else {
        Variant::ClassicImageLight
    };
    let texture = variant_texture(
        ctx,
        icon,
        physical_size(points, ctx.pixels_per_point()),
        variant,
    );
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
        assert_eq!(Icon::ALL.len(), 96);
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
                assert_eq!(
                    cache.entries.back().unwrap().0,
                    (Icon::ColorBalance, size, Variant::Sheet)
                );
            }
            let first = texture(&ctx, Icon::ColorBalance, size);
            let again = texture(&ctx, Icon::ColorBalance, size);
            assert_eq!(first.id(), again.id());
        }
    }

    #[test]
    fn classic_shape_vectors_are_monochrome_outlines_at_each_density() {
        let shapes: Vec<_> = Icon::ALL
            .into_iter()
            .filter(|icon| classic_shapes::source(*icon).is_some())
            .collect();
        assert_eq!(shapes.len(), 23);
        for icon in shapes {
            let source = std::str::from_utf8(classic_shapes::source(icon).unwrap()).unwrap();
            for forbidden in ["<image", "href=", "<text", "Gradient"] {
                assert!(!source.contains(forbidden), "{}: {forbidden}", icon.name());
            }
            for size in [24, 48, 96, 128] {
                let image = rasterize_variant(icon, size, Variant::ClassicShape);
                assert!(image.pixels.iter().any(|pixel| pixel.a() > 0));
                assert!(image.pixels.iter().any(|pixel| pixel.a() == 0));
                for pixel in &image.pixels {
                    let [r, g, b, a] = pixel.to_array();
                    assert_eq!([r, g, b], [a; 3], "{} must be tintable white", icon.name());
                }
                if matches!(
                    icon,
                    Icon::Rectangle | Icon::RoundedRectangle | Icon::Ellipse
                ) {
                    assert_eq!(
                        image.pixels[(size * size / 2 + size / 2) as usize].a(),
                        0,
                        "{} must have an empty center",
                        icon.name()
                    );
                }
            }
        }
        let line = rasterize_variant(Icon::Line, 24, Variant::ClassicShape);
        assert!(line.pixels[4 * 24 + 4].a() > 0);
        assert_eq!(line.pixels[19 * 24 + 4].a(), 0);
    }

    #[test]
    fn colored_and_classic_shape_textures_have_independent_cache_identity() {
        let ctx = Context::default();
        let colored = texture(&ctx, Icon::Rectangle, 48);
        let outline = variant_texture(&ctx, Icon::Rectangle, 48, Variant::ClassicShape);
        assert_ne!(colored.id(), outline.id());
        assert_eq!(texture(&ctx, Icon::Rectangle, 48).id(), colored.id());
        assert_eq!(
            variant_texture(&ctx, Icon::Rectangle, 48, Variant::ClassicShape).id(),
            outline.id()
        );
    }

    #[test]
    fn classic_menu_vectors_are_monochrome_and_do_not_alias_colored_textures() {
        let ctx = Context::default();
        let menus: Vec<_> = Icon::ALL
            .into_iter()
            .filter(|icon| classic_menus::source(*icon).is_some())
            .collect();
        assert_eq!(menus.len(), 28);
        for icon in menus {
            let source = std::str::from_utf8(classic_menus::source(icon).unwrap()).unwrap();
            for forbidden in ["<image", "href=", "<text", "Gradient"] {
                assert!(!source.contains(forbidden), "{}: {forbidden}", icon.name());
            }
            for size in [24, 48, 96, 128] {
                let pixels = rasterize_variant(icon, size, Variant::ClassicMenu).pixels;
                assert!(pixels.iter().any(|pixel| pixel.a() > 0));
                assert!(pixels.iter().any(|pixel| pixel.a() == 0));
                for pixel in pixels {
                    let [r, g, b, a] = pixel.to_array();
                    assert_eq!(
                        [r, g, b],
                        [a; 3],
                        "{} must remain tintable white",
                        icon.name()
                    );
                }
            }
            let colored = texture(&ctx, icon, 48);
            let outline = variant_texture(&ctx, icon, 48, Variant::ClassicMenu);
            assert_ne!(colored.id(), outline.id());
            assert_eq!(
                variant_texture(&ctx, icon, 48, Variant::ClassicMenu).id(),
                outline.id()
            );
        }
        let cache = ctx
            .data_mut(|data| {
                data.get_temp::<Arc<egui::mutex::Mutex<Cache>>>(egui::Id::unique(
                    "paintcraft-svg-icons",
                ))
            })
            .unwrap();
        assert!(cache.lock().entries.len() <= CACHE_ENTRIES);
        assert!(cache.lock().bytes <= CACHE_BYTES);
    }

    #[test]
    fn classic_image_art_retains_accent_coverage_and_independent_theme_textures() {
        let ctx = Context::default();
        let icons: Vec<_> = Icon::ALL
            .into_iter()
            .filter(|icon| classic_image::source(*icon, false).is_some())
            .collect();
        assert_eq!(icons.len(), 9);
        for icon in icons {
            for dark in [false, true] {
                let source =
                    std::str::from_utf8(classic_image::source(icon, dark).unwrap()).unwrap();
                for forbidden in ["<image", "href=", "<text", "Gradient"] {
                    assert!(!source.contains(forbidden));
                }
            }

            for size in [24, 48, 96, 128] {
                let light = rasterize_variant(icon, size, Variant::ClassicImageLight);
                let dark = rasterize_variant(icon, size, Variant::ClassicImageDark);
                assert!(light.pixels.iter().any(|pixel| pixel.a() > 0));
                assert!(light.pixels.iter().any(|pixel| pixel.a() == 0));
                for (a, b) in light.pixels.iter().zip(&dark.pixels) {
                    assert_eq!(a.a(), b.a(), "{} theme coverage must agree", icon.name());
                }
                assert!(
                    light.pixels.iter().zip(&dark.pixels).any(|(a, b)| a != b),
                    "theme ink must change"
                );
                if icon != Icon::Crop {
                    for image in [&light, &dark] {
                        assert!(
                            image
                                .pixels
                                .iter()
                                .any(|pixel| pixel.b() > pixel.g() && pixel.g() > pixel.r()),
                            "{} retains its blue accent",
                            icon.name()
                        );
                    }
                }
            }
            let light = variant_texture(&ctx, icon, 48, Variant::ClassicImageLight);
            let dark = variant_texture(&ctx, icon, 48, Variant::ClassicImageDark);
            assert_ne!(light.id(), dark.id());
            if classic_menus::source(icon).is_some() {
                let menu = variant_texture(&ctx, icon, 48, Variant::ClassicMenu);
                assert_ne!(light.id(), menu.id());
                assert_ne!(dark.id(), menu.id());
            }
            assert_ne!(light.id(), texture(&ctx, icon, 48).id());
            assert_eq!(
                light.id(),
                variant_texture(&ctx, icon, 48, Variant::ClassicImageLight).id()
            );
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
