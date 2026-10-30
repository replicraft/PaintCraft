//! Captured browser encoding choices, independent of download ownership.
use super::*;
#[cfg(test)]
pub(super) fn encode(
    document: &Document,
    format: Option<ImageExport>,
    jpeg_quality: u8,
    frontend: Frontend,
    pcx_encoding: paintcraft_ui::pcx_export::Encoding,
    jpeg_matte: Option<image::JpegMatte>,
    source_metadata: paintcraft_formats::export::metadata_policy::Intent,
) -> Result<Vec<u8>> {
    encode_with_options(
        document,
        ExportOptions {
            format,
            jpeg_quality,
            frontend,
            pcx_encoding,
            jpeg_matte,
            source_metadata,
            filter_bake: Default::default(),
        },
    )
}
pub(super) fn encode_with_options(document: &Document, options: ExportOptions) -> Result<Vec<u8>> {
    let ExportOptions {
        format,
        jpeg_quality,
        frontend,
        pcx_encoding,
        jpeg_matte,
        source_metadata,
        filter_bake,
    } = options;
    if jpeg_matte.is_some() && format != Some(ImageExport::Jpeg) {
        return Err(Error::new(
            ErrorCode::InputSchema,
            "JPEG matte requires JPEG output",
        ));
    }
    if let Some(format) = format {
        let format = match format {
            ImageExport::Png => image::ImageFormat::Png,
            ImageExport::Jpeg => image::ImageFormat::Jpeg,
            ImageExport::Bmp => image::ImageFormat::Bmp,
            ImageExport::Gif => image::ImageFormat::Gif,
            ImageExport::Tiff => image::ImageFormat::Tiff,
            ImageExport::Tga => image::ImageFormat::Tga,
            ImageExport::Webp => image::ImageFormat::WebP,
            ImageExport::Jxl => image::ImageFormat::Jxl,
            ImageExport::Dds => image::ImageFormat::Dds,
            ImageExport::Avif => image::ImageFormat::Avif,
            ImageExport::Qoi => image::ImageFormat::Qoi,
            ImageExport::Pam => image::ImageFormat::Pnm,
            ImageExport::Farbfeld => image::ImageFormat::Farbfeld,
            ImageExport::Pcx => image::ImageFormat::Pcx,
            ImageExport::Msp => image::ImageFormat::Msp,
        };
        // The captured frontend determines admission before rendering or encoding.
        use paintcraft_formats::registry::{self, Operation, Request};
        let family = Family::from_id(format.name()).ok_or_else(|| {
            Error::new(
                ErrorCode::CapabilityUnsupported,
                "Unregistered export format",
            )
        })?;
        let request = Request::new(frontend, Operation::ExportFlat, family, Platform::Web);
        let decision = registry::admission(request);
        if !decision.available() {
            decision.require_allowed(request)?;
        }
        let mut options = paintcraft_formats::export::FlatOptions::new(format, jpeg_quality);
        options.jpeg_matte = jpeg_matte;
        options.filter_bake = filter_bake;
        options.source_metadata = paintcraft_formats::export::metadata_policy::Policy::capture(
            document,
            paintcraft_formats::export::metadata_policy::Target::new(family, Operation::ExportFlat),
            source_metadata,
        )?;
        if format == image::ImageFormat::Pcx {
            options.pcx_mode = pcx_mode(pcx_encoding);
        }
        return Ok(paintcraft_formats::export::export_flat_options(
            document,
            options,
            &paintcraft_formats::export::KNOWN_LOSSES.map(str::to_owned),
        )?
        .bytes);
    }
    // Project saves always preserve attachments. Even an empty project must not
    // silently accept a request to omit them from PCND.
    paintcraft_formats::export::metadata_policy::Policy::capture(
        document,
        paintcraft_formats::export::metadata_policy::Target::new(
            Family::Pcnd,
            paintcraft_formats::registry::Operation::SavePreserving,
        ),
        source_metadata,
    )?;
    let mut writer = Limited(Vec::new());
    pcnd::write_compact_to(document, &mut writer)?;
    Ok(writer.0)
}
