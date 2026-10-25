//! Flat image export of a document, with every loss named and acknowledged.

use paintcraft_core::types::{Error, ErrorCode, Result};
use paintcraft_document::Document;
use paintcraft_render::render_flat;

use crate::image::{self, ImageFormat};

/// Loss codes a caller may acknowledge.
pub const KNOWN_LOSSES: [&str; 7] = [
    "flatten",
    "hidden-rgb",
    "lossy-jpeg",
    "alpha-discarded",
    "palette-quantized",
    "alpha-binarized",
    "transparent-rgb-discarded",
];

#[derive(Debug)]
pub struct FlatExport {
    pub bytes: Vec<u8>,
    /// Exactly the losses incurred; all were acknowledged.
    pub losses: Vec<String>,
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
    check_loss_codes(allowed)?;
    let rgba = render(doc)?;
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
    losses.extend(
        format
            .export_losses_for_rgba(&rgba)
            .into_iter()
            .map(str::to_owned),
    );
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
                format.name(),
                missing.join(", "),
                missing.join(",")
            ),
        ));
    }
    let bytes = image::encode(format, doc.canvas.0, doc.canvas.1, &rgba, jpeg_quality)?;
    Ok(FlatExport { bytes, losses })
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
