//! Real stock decoder entrypoints; no editor, filesystem, helper or GPU work.
use crate::{PIXELS_MAX, Target};
use paintcraft_core::types::{Error, ErrorCode, Result};
use paintcraft_formats::{icons, image, tiff_pages};
mod bmp;
mod png;

fn invariant(message: &str) -> Error {
    Error::new(ErrorCode::InvariantViolation, message)
}
fn samples(width: u32, height: u32, rgba: &[u8]) -> Result<()> {
    let count = u64::from(width) * u64::from(height);
    if width == 0 || height == 0 || count > PIXELS_MAX || count * 4 != rgba.len() as u64 {
        return Err(invariant("stock decoder output geometry/extent"));
    }
    Ok(())
}
pub(crate) fn exercise(target: Target, bytes: &[u8], check: &dyn Fn() -> Result<()>) -> Result<()> {
    match target {
        Target::Raster => {
            if !matches!(
                image::ImageFormat::sniff(bytes),
                Some(
                    image::ImageFormat::Png
                        | image::ImageFormat::Jpeg
                        | image::ImageFormat::Gif
                        | image::ImageFormat::Bmp
                        | image::ImageFormat::Tiff
                )
            ) {
                return Err(Error::new(
                    ErrorCode::CapabilityUnsupported,
                    "stock fuzz raster family",
                ));
            }
            // This facade has a public pixel budget, not a callback parameter.
            let decoded = image::decode_with_pixel_limit(bytes, PIXELS_MAX)?;
            samples(decoded.width, decoded.height, &decoded.rgba)?;
            if decoded
                .losses
                .iter()
                .any(|loss| !image::KNOWN_IMPORT_LOSSES.contains(&loss.as_str()))
            {
                return Err(invariant("stock decoder emitted unknown conversion loss"));
            }
            if decoded.format == image::ImageFormat::Png {
                png::exercise(bytes, &decoded, check)?;
            } else if decoded.format == image::ImageFormat::Bmp {
                bmp::exercise(bytes, &decoded, check)?;
            }
        }
        Target::TiffPages => {
            let inventory = tiff_pages::inspect(bytes, check)?;
            let eligible: Vec<_> = inventory
                .pages()
                .iter()
                .filter(|p| p.unsupported_reason.is_none())
                .collect();
            let first = eligible.first().ok_or_else(|| {
                Error::new(
                    ErrorCode::CapabilityUnsupported,
                    "no qualified TIFF fuzz page",
                )
            })?;
            let mut selected = vec![first.index];
            if let Some(last) = eligible.last()
                && last.index != first.index
            {
                selected.push(last.index);
            }
            for index in selected {
                check()?;
                let decoded = tiff_pages::decode_page(bytes, index, PIXELS_MAX, check)?;
                samples(decoded.width, decoded.height, &decoded.rgba)?;
                let report = tiff_pages::Assessment::decode(&decoded.assessment.encode())
                    .map_err(|_| invariant("emitted TIFF assessment cannot round trip"))?;
                report
                    .validate_result(
                        inventory.source_sha256(),
                        inventory.source_bytes(),
                        index,
                        decoded.width,
                        decoded.height,
                        &decoded.rgba,
                    )
                    .map_err(|_| invariant("TIFF assessment source/page/output binding"))?;
                if report.page_count() as usize != inventory.pages().len()
                    || report.descriptor() != &inventory.pages()[index as usize].descriptor
                    || decoded
                        .losses
                        .iter()
                        .map(String::as_str)
                        .collect::<Vec<_>>()
                        != report.losses()
                {
                    return Err(invariant(
                        "TIFF assessment descriptor/loss inventory binding",
                    ));
                }
            }
        }
        Target::Icons => {
            let directory = icons::inspect(bytes)?;
            let mut selected = vec![0];
            if directory.entries.len() > 1 {
                selected.push((directory.entries.len() - 1) as u16);
            }
            for index in selected {
                check()?;
                // Like the raster facade, this API admits pixels without an
                // interruptible-library-call or hard-time contract.
                let decoded = icons::decode_entry(bytes, index, PIXELS_MAX)?;
                samples(decoded.entry.width, decoded.entry.height, &decoded.rgba)?;
                if decoded.entry != directory.entries[index as usize]
                    || decoded
                        .losses
                        .iter()
                        .any(|loss| !icons::KNOWN_IMPORT_LOSSES.contains(&loss.as_str()))
                {
                    return Err(invariant("icon selected entry/loss inventory binding"));
                }
            }
        }
        _ => return Err(invariant("non-stock target reached stock dispatch")),
    }
    check()
}

#[cfg(test)]
mod tests;
