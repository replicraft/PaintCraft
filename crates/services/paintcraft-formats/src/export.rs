//! Flat image export of a document, with every loss named and acknowledged.

use paintcraft_core::types::{Error, ErrorCode, Result};
use paintcraft_document::Document;
use paintcraft_render::render_flat;

use crate::image::{self, ImageFormat};
pub mod dds;
pub mod jxr;

/// Loss codes a caller may acknowledge.
pub const KNOWN_LOSSES: [&str; 12] = [
    "flatten",
    "hidden-rgb",
    "lossy-jpeg",
    "lossy-avif",
    "alpha-lossy",
    "alpha-discarded",
    "palette-quantized",
    "alpha-binarized",
    "transparent-rgb-discarded",
    crate::dds::BLOCK_COMPRESSION_LOSS,
    crate::dds::MIP_HIDDEN_RGB_LOSS,
    dds::LINEAR_QUANTIZATION_LOSS,
];

#[derive(Debug)]
pub struct FlatExport {
    pub bytes: Vec<u8>,
    /// Exactly the losses incurred; all were acknowledged.
    pub losses: Vec<String>,
}

#[derive(Clone, Copy, Debug)]
pub struct FlatOptions {
    pub format: ImageFormat,
    pub quality: u8,
    pub pcx_mode: crate::pcx::Mode,
}
impl FlatOptions {
    pub fn new(format: ImageFormat, quality: u8) -> Self {
        Self {
            format,
            quality,
            pcx_mode: crate::pcx::Mode::Rgb24,
        }
    }
}

pub fn export_flat_options(
    doc: &Document,
    options: FlatOptions,
    allowed: &[String],
) -> Result<FlatExport> {
    export_flat_options_with(doc, options, allowed, render_flat)
}

/// Reject unknown loss codes up front, before any work.
pub fn check_loss_codes(codes: &[String]) -> Result<()> {
    match codes.iter().find(|c| !KNOWN_LOSSES.contains(&c.as_str())) {
        Some(bad) => Err(Error::new(
            ErrorCode::InputSchema,
            format!(
                "unknown loss code {bad:?}; known: {}",
                KNOWN_LOSSES.join(", ")
            ),
        )),
        None => Ok(()),
    }
}

/// Render the composite and encode it. Fails with `capability.unsupported`,
/// naming every missing acknowledgement, if the export would lose anything the
/// caller did not allow.
pub fn export_flat(
    doc: &Document,
    format: ImageFormat,
    jpeg_quality: u8,
    allowed: &[String],
) -> Result<FlatExport> {
    export_flat_with(doc, format, jpeg_quality, allowed, render_flat)
}

/// Export through a caller-owned exact default-v1 renderer, preserving all loss
/// checks. This supports bounded CPU pools without owning application scheduling.
pub fn export_flat_with(
    doc: &Document,
    format: ImageFormat,
    jpeg_quality: u8,
    allowed: &[String],
    render: impl FnOnce(&Document) -> Result<Vec<u8>>,
) -> Result<FlatExport> {
    export_flat_options_with(doc, FlatOptions::new(format, jpeg_quality), allowed, render)
}

pub fn export_flat_options_with(
    doc: &Document,
    options: FlatOptions,
    allowed: &[String],
    render: impl FnOnce(&Document) -> Result<Vec<u8>>,
) -> Result<FlatExport> {
    export_flat_options_with_check(doc, options, allowed, render, &|| Ok(()))
}

/// Preserve the shared flat policy while checking caller cancellation in conversion
/// and encoding. A caller-owned renderer must check during its own work.
pub fn export_flat_options_with_check(
    doc: &Document,
    options: FlatOptions,
    allowed: &[String],
    render: impl FnOnce(&Document) -> Result<Vec<u8>>,
    check: &dyn Fn() -> Result<()>,
) -> Result<FlatExport> {
    check()?;
    let FlatOptions {
        format,
        quality: jpeg_quality,
        pcx_mode,
    } = options;
    check_loss_codes(allowed)?;
    if format != ImageFormat::Pcx && pcx_mode != crate::pcx::Mode::Rgb24 {
        return Err(Error::new(
            ErrorCode::InputSchema,
            "PCX encoding options require PCX output",
        ));
    }
    let FlatComposite { rgba, mut losses } = prepare_composite(doc, render, check)?;
    losses.extend(
        format
            .export_losses_for_rgba(&rgba)
            .into_iter()
            .map(str::to_owned),
    );
    let prepared = if format == ImageFormat::Pcx && pcx_mode != crate::pcx::Mode::Rgb24 {
        Some(crate::pcx::prepare_rgba(
            doc.canvas.0,
            doc.canvas.1,
            &rgba,
            pcx_mode,
            check,
        )?)
    } else {
        None
    };
    if prepared
        .as_ref()
        .is_some_and(|prepared| prepared.is_quantized())
    {
        losses.push("palette-quantized".into());
    }
    require_losses(format.name(), &losses, allowed)?;
    let bytes = if let Some(prepared) = prepared {
        prepared.encode(check)?
    } else {
        image::encode_checked(
            format,
            doc.canvas.0,
            doc.canvas.1,
            &rgba,
            jpeg_quality,
            check,
        )?
    };
    Ok(FlatExport { bytes, losses })
}

/// One immutable render and the same format-independent losses used by all flat exports.
struct FlatComposite {
    rgba: Vec<u8>,
    losses: Vec<String>,
}
fn prepare_composite(
    doc: &Document,
    render: impl FnOnce(&Document) -> Result<Vec<u8>>,
    check: &dyn Fn() -> Result<()>,
) -> Result<FlatComposite> {
    let rgba = render(doc)?;
    check()?;
    if rgba.len() as u128 != u128::from(doc.canvas.0) * u128::from(doc.canvas.1) * 4 {
        return Err(Error::new(
            ErrorCode::InvariantViolation,
            "export renderer returned an invalid pixel buffer",
        ));
    }
    let mut losses: Vec<String> = Vec::new();
    if doc.roots.len() > 1 {
        losses.push("flatten".into());
    }
    if doc.layers.values().any(|l| l.raster.has_hidden_rgb()) {
        losses.push("hidden-rgb".into());
    }
    Ok(FlatComposite { rgba, losses })
}
fn require_losses(label: &str, losses: &[String], allowed: &[String]) -> Result<()> {
    let missing: Vec<&str> = losses
        .iter()
        .filter(|l| !allowed.contains(l))
        .map(String::as_str)
        .collect();
    if !missing.is_empty() {
        return Err(Error::new(
            ErrorCode::CapabilityUnsupported,
            format!(
                "{} export would lose: {}; acknowledge with --allow-loss {}",
                label,
                missing.join(", "),
                missing.join(",")
            ),
        ));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use paintcraft_core::types::Rgba;
    use paintcraft_document::{POLICY, new_document};
    use paintcraft_editor::session::Session;

    fn doc_with(ops: &str) -> Document {
        let mut s =
            Session::new(new_document(4, 4, Some(Rgba([255, 255, 255, 255])), 0, POLICY).unwrap());
        let t = s.ticket();
        let text = format!(
            r#"{{"schema":"paintcraft.batch/v2","atomic":true,"policy":"{POLICY}","base":{{"state_id":"{}","publication_epoch":"{}"}},"operations":{ops}}}"#,
            t.state_id, t.publication_epoch
        );
        s.apply_json(text.as_bytes()).unwrap();
        s.document().clone()
    }

    fn allow(codes: &[&str]) -> Vec<String> {
        codes.iter().map(|c| (*c).to_owned()).collect()
    }

    #[test]
    fn a_single_opaque_layer_exports_to_png_and_bmp_without_acknowledgement() {
        let d = new_document(4, 4, Some(Rgba([1, 2, 3, 255])), 0, POLICY).unwrap();
        for f in [ImageFormat::Png, ImageFormat::Bmp] {
            let e = export_flat(&d, f, 90, &[]).unwrap();
            assert!(e.losses.is_empty());
            assert_eq!(image::decode(&e.bytes).unwrap().rgba[..4], [1, 2, 3, 255]);
        }
    }

    #[test]
    fn every_loss_is_named_and_must_be_acknowledged() {
        let d = doc_with(
            r##"[{"op":"layer.add","ref":"a","name":"A","width":4,"height":4,"origin":{"x":0,"y":0},"default_pixel":"#12345600"}]"##,
        );
        let e = export_flat(&d, ImageFormat::Png, 90, &[]).unwrap_err();
        assert_eq!(e.code, ErrorCode::CapabilityUnsupported);
        assert!(
            e.message.contains("flatten") && e.message.contains("hidden-rgb"),
            "{}",
            e.message
        );
        let ok = export_flat(&d, ImageFormat::Png, 90, &allow(&["flatten", "hidden-rgb"])).unwrap();
        assert_eq!(ok.losses, ["flatten", "hidden-rgb"]);
        // Allowing more than is needed does not report losses that did not happen.
        let one = export_flat(
            &new_document(2, 2, None, 0, POLICY).unwrap(),
            ImageFormat::Png,
            90,
            &allow(&KNOWN_LOSSES),
        )
        .unwrap();
        assert!(one.losses.is_empty());
    }

    #[test]
    fn jpeg_needs_lossy_and_alpha_acknowledgement_only_when_transparent() {
        let opaque = new_document(4, 4, Some(Rgba([9, 9, 9, 255])), 0, POLICY).unwrap();
        assert!(export_flat(&opaque, ImageFormat::Jpeg, 90, &[]).is_err());
        assert_eq!(
            export_flat(&opaque, ImageFormat::Jpeg, 90, &allow(&["lossy-jpeg"]))
                .unwrap()
                .losses,
            ["lossy-jpeg"]
        );
        let clear = new_document(4, 4, None, 0, POLICY).unwrap();
        assert!(export_flat(&clear, ImageFormat::Jpeg, 90, &allow(&["lossy-jpeg"])).is_err());
        assert_eq!(
            export_flat(
                &clear,
                ImageFormat::Jpeg,
                90,
                &allow(&["lossy-jpeg", "alpha-discarded"])
            )
            .unwrap()
            .losses,
            ["lossy-jpeg", "alpha-discarded"]
        );
    }

    #[test]
    fn unknown_loss_codes_are_invocation_errors() {
        assert!(check_loss_codes(&allow(&["flatten", "lossy-jpeg"])).is_ok());
        assert_eq!(
            check_loss_codes(&allow(&["bogus"])).unwrap_err().code,
            ErrorCode::InputSchema
        );
    }
}
