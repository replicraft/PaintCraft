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
                | Descriptor::CurvesRgbV1(_) => true,
            })
    })
}
pub(super) fn cli_schema(doc: &Document) -> &'static str {
    if !paintcraft_editor::session::catalog_document_requirements(doc).is_empty() {
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
    if !paintcraft_editor::session::catalog_document_requirements(doc).is_empty() {
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
