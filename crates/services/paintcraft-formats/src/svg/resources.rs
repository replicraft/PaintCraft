use super::*;
use base64::{Engine, engine::general_purpose::STANDARD};

#[derive(Default)]
pub(super) struct Images(Vec<(String, Vec<u8>, usvg::ImageKind)>);
impl Images {
    pub fn resolver(&self) -> usvg::ImageHrefResolver<'_> {
        usvg::ImageHrefResolver {
            resolve_string: Box::new(|_, _| None),
            resolve_data: Box::new(|mime, data, _| {
                self.0
                    .iter()
                    .find(|(kind, original, _)| {
                        kind == mime && original.as_slice() == data.as_slice()
                    })
                    .map(|(_, _, kind)| kind.clone())
            }),
        }
    }
}
pub(super) fn prepare(
    urls: &[&str],
    pixel_limit: u64,
    check: &dyn Fn() -> Result<()>,
) -> Result<(Images, Vec<String>)> {
    let mut images = Images::default();
    let mut losses = Vec::new();
    let mut pixels = 0u64;
    let mut encoded = 0usize;
    for url in urls {
        check()?;
        let (kind, payload) = url
            .strip_prefix("data:")
            .and_then(|s| s.split_once(";base64,"))
            .ok_or_else(|| unsupported("SVG images require an embedded base64 raster data URL"))?;
        if !["image/png", "image/jpeg", "image/gif", "image/webp"].contains(&kind) {
            return Err(unsupported("SVG embedded image type is not qualified"));
        }
        if payload.len() > SOURCE_MAX {
            return Err(resource("SVG embedded image exceeds encoded allowance"));
        }
        let original = STANDARD
            .decode(payload)
            .map_err(|_| malformed("invalid SVG embedded image base64"))?;
        if images
            .0
            .iter()
            .any(|(mime, bytes, _)| mime == kind && *bytes == original)
        {
            continue;
        }
        let decoded = crate::image::decode_with_pixel_limit(
            &original,
            pixel_limit.min(PIXELS_MAX).saturating_sub(pixels),
        )?;
        let expected_mime = match decoded.format {
            crate::image::ImageFormat::Png => "image/png",
            crate::image::ImageFormat::Jpeg => "image/jpeg",
            crate::image::ImageFormat::Gif => "image/gif",
            crate::image::ImageFormat::WebP => "image/webp",
            _ => {
                return Err(unsupported(
                    "SVG embedded image signature is not a qualified raster",
                ));
            }
        };
        if kind != expected_mime {
            return Err(malformed("SVG embedded image MIME and signature disagree"));
        }
        pixels += u64::from(decoded.width) * u64::from(decoded.height);
        for code in decoded.losses {
            loss(&mut losses, &code);
        }
        // Only a validated, metadata-free private PNG reaches resvg's decoder.
        let png = crate::image::encode_checked(
            crate::image::ImageFormat::Png,
            decoded.width,
            decoded.height,
            &decoded.rgba,
            100,
            check,
        )?;
        encoded += original.len() + png.len();
        if encoded > 32 << 20 {
            return Err(resource("SVG embedded image cache exceeds allowance"));
        }
        images.0.push((
            kind.to_owned(),
            original,
            usvg::ImageKind::PNG(Arc::new(png)),
        ));
    }
    Ok((images, losses))
}
