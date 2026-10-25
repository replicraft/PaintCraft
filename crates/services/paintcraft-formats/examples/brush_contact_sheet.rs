//! Actual shared brush coverage, PNG pixels and SHA256 recipe without a window.
use paintcraft_core::types::{Rect, Rgba, brush::BrushMedia};
use paintcraft_formats::image::{ImageFormat, encode};
use paintcraft_raster::{font, media, smooth};
use sha2::{Digest, Sha256};
use std::{fs, path::Path};

const WIDTH: usize = 1860;
const HEIGHT: usize = 1150;
const SIZE: u32 = 18;

fn label(pixels: &mut [Rgba], at: (usize, usize), text: &str) {
    for (index, c) in text.chars().enumerate() {
        if let Some(rows) = font::glyph(c) {
            for (y, row) in rows.into_iter().enumerate() {
                for x in 0..8 {
                    if row & (1 << x) != 0 {
                        pixels[(at.1 + y) * WIDTH + at.0 + index * 8 + x] = Rgba([35, 35, 35, 255]);
                    }
                }
            }
        }
    }
}

fn sheet(background: Rgba) -> Result<Vec<Rgba>, Box<dyn std::error::Error>> {
    let mut pixels = vec![background; WIDTH * HEIGHT];
    let clip = Rect::new(0, 0, WIDTH as i64, HEIGHT as i64).ok_or("invalid sheet dimensions")?;
    label(
        &mut pixels,
        (12, 12),
        "PaintCraft actual raster: long straight / curve / retrace; size 18 px",
    );
    for (column, opacity) in [255u8, 128, 64].into_iter().enumerate() {
        let left = 210 + column * 550;
        label(
            &mut pixels,
            (left, 34),
            ["100% opacity", "50% opacity", "25% opacity"][column],
        );
        for (row, medium) in BrushMedia::ALL.into_iter().enumerate() {
            let top = 60 + row * 120;
            if column == 0 {
                label(&mut pixels, (12, top + 47), medium.name());
            }
            let x0 = left as i64 + 12;
            let y0 = top as i64;
            let straight = vec![(x0, y0 + 15), (x0 + 510, y0 + 15)];
            let curve = (0..=20)
                .map(|i| {
                    (
                        x0 + i * 25,
                        y0 + 54 + (i as f64 * 0.35).sin().mul_add(12.0, 0.0).round() as i64,
                    )
                })
                .collect();
            let retrace = vec![(x0, y0 + 95), (x0 + 510, y0 + 95), (x0, y0 + 95)];
            for points in [straight, curve, retrace] {
                let spans = if medium == BrushMedia::Plain {
                    smooth::round_path(&points, SIZE, &clip)?
                        .into_iter()
                        .map(|s| {
                            (
                                s.y,
                                s.x0,
                                s.x1,
                                ((u16::from(s.samples) * 255 + 8) / 16) as u8,
                            )
                        })
                        .collect::<Vec<_>>()
                } else {
                    media::coverage(&points, SIZE, 1, medium, clip, &|| Ok(()))?
                        .into_iter()
                        .map(|s| (s.y, s.x0, s.x1, s.coverage))
                        .collect()
                };
                for (y, x0, x1, coverage) in spans {
                    let color = Rgba([12, 80, 176, opacity]).with_opacity(coverage);
                    for x in x0..=x1 {
                        let target = &mut pixels[y as usize * WIDTH + x as usize];
                        *target = color.source_over(*target);
                    }
                }
            }
        }
    }
    Ok(pixels)
}

fn save(path: &Path, name: &str, pixels: &[Rgba]) -> Result<String, Box<dyn std::error::Error>> {
    let rgba: Vec<_> = pixels.iter().flat_map(|p| p.0).collect();
    fs::write(path.join(format!("{name}.rgba")), &rgba)?;
    fs::write(
        path.join(format!("{name}.png")),
        encode(ImageFormat::Png, WIDTH as u32, HEIGHT as u32, &rgba, 100)?,
    )?;
    let hash: String = Sha256::digest(&rgba)
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect();
    Ok(format!("{name}.rgba sha256={hash}\n"))
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let destination = std::env::args().nth(1).ok_or("expected output directory")?;
    let path = Path::new(&destination);
    fs::create_dir_all(path)?;
    let mut recipe = format!(
        "width={WIDTH}\nheight={HEIGHT}\nsize={SIZE}\npoint_scale=1\nink=#0C50B0\nopacity_u8=255,128,64\nrow_media=BrushMedia::ALL\nrecipe=crates/services/paintcraft-formats/examples/brush_contact_sheet.rs\n"
    );
    recipe.push_str(&save(path, "white", &sheet(Rgba([255, 255, 255, 255]))?)?);
    let transparent = sheet(Rgba::TRANSPARENT)?;
    recipe.push_str(&save(path, "transparent", &transparent)?);
    let checker = transparent
        .into_iter()
        .enumerate()
        .map(|(i, p)| {
            let gray = if ((i % WIDTH) / 10 + (i / WIDTH) / 10).is_multiple_of(2) {
                230
            } else {
                255
            };
            p.source_over(Rgba([gray, gray, gray, 255]))
        })
        .collect::<Vec<_>>();
    recipe.push_str(&save(path, "checker", &checker)?);
    fs::write(path.join("recipe.txt"), recipe)?;
    Ok(())
}
