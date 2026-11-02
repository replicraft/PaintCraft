//! Minimal inspection profile only; result and persistence profiles stay separate.
use paintcraft_core::types::filters::Descriptor;
use paintcraft_document::Document;

pub(super) fn spatial(doc: &Document) -> bool {
    doc.layers.values().any(|layer| {
        layer
            .filters
            .iter()
            .any(|instance| instance.descriptor().is_spatial())
    })
}
pub(super) fn expanded(doc: &Document) -> bool {
    doc.layers.values().any(|layer| {
        layer
            .filters
            .iter()
            .any(|instance| match instance.descriptor() {
                Descriptor::BoxBlurV1(_)
                | Descriptor::InvertRgbV1
                | Descriptor::BrightnessContrastV1(_) => false,
                Descriptor::BlackWhiteRgbV1(_)
                | Descriptor::ThresholdLumaV1(_)
                | Descriptor::PosterizeRgbV1(_)
                | Descriptor::SolarizeRgbV1
                | Descriptor::LevelsRgbV1(_)
                | Descriptor::ExposureSrgbV1(_)
                | Descriptor::CurvesRgbV1(_)
                | Descriptor::VibranceRgbV1(_)
                | Descriptor::HueSaturationRgbV1(_)
                | Descriptor::AddNoiseRgbV1(_) => true,
            })
    })
}
pub(super) fn cli_schema(doc: &Document) -> &'static str {
    if doc.has_layer_masks() {
        "paintcraft.inspect/v5"
    } else if !paintcraft_editor::session::catalog_document_requirements(doc).is_empty() {
        "paintcraft.inspect/catalog-v1"
    } else if spatial(doc) {
        "paintcraft.inspect/v4"
    } else if expanded(doc) {
        "paintcraft.inspect/v3"
    } else if paintcraft_editor::session::masked_document_features(doc).is_empty() {
        "paintcraft.inspect/v1"
    } else {
        "paintcraft.inspect/v2"
    }
}
pub(super) fn mcp_schema(doc: &Document, masked: bool) -> Option<&'static str> {
    if doc.has_layer_masks() {
        Some("paintcraft.mcp-inspect/v5")
    } else if !paintcraft_editor::session::catalog_document_requirements(doc).is_empty() {
        Some("paintcraft.mcp-inspect/catalog-v1")
    } else if spatial(doc) {
        Some("paintcraft.mcp-inspect/v4")
    } else if expanded(doc) {
        Some("paintcraft.mcp-inspect/v3")
    } else if masked {
        Some("paintcraft.mcp-inspect/v2")
    } else {
        None
    }
}

/// Old inspection profiles cannot truthfully disclose stored composition intent.
pub(super) fn require_supported(doc: &Document) -> paintcraft_core::types::Result<()> {
    doc.require_normal_blends()
}
pub(super) fn require_source(bytes: &[u8]) -> paintcraft_core::types::Result<()> {
    if bytes.len() >= 8 && &bytes[..8] == b"PCND\x08\r\n\x1a" {
        return Err(paintcraft_core::types::Error::new(
            paintcraft_core::types::ErrorCode::CapabilityUnsupported,
            "PCND8 requires a new blend-aware inspection profile",
        ));
    }
    Ok(())
}

#[cfg(test)]
mod blend_closed_tests {
    use super::*;
    #[test]
    fn known_pcnd8_and_stored_hidden_modes_are_closed_to_old_inspection() {
        assert_eq!(
            require_source(b"PCND\x08\r\n\x1a").unwrap_err().code,
            paintcraft_core::types::ErrorCode::CapabilityUnsupported
        );
        for version in 1..=7 {
            let mut bytes = *b"PCND\x01\r\n\x1a";
            bytes[4] = version;
            require_source(&bytes).unwrap();
        }
        let mut doc = paintcraft_document::new_document(
            1,
            1,
            Some(paintcraft_core::types::Rgba([1, 2, 3, 0])),
            0,
            paintcraft_document::POLICY,
        )
        .unwrap();
        let layer = doc.layers.get_mut(&doc.roots[0]).unwrap();
        layer.visible = false;
        layer.opacity = 0;
        layer.blend_mode = paintcraft_core::types::LayerBlendMode::Multiply;
        assert_eq!(
            require_supported(&doc).unwrap_err().code,
            paintcraft_core::types::ErrorCode::CapabilityUnsupported
        );
    }
}
