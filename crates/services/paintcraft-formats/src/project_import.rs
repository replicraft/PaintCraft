//! Explicit converted copies of foreign layered documents, never preserving saves.
use paintcraft_core::types::{Error, ErrorCode, Result, Rgba, ids::deterministic_id};
use paintcraft_document::{Document, Layer};
use paintcraft_raster::{Admission, RasterLayer};
use sha2::{Digest, Sha256};
use std::collections::HashMap;
mod ora;
mod psd;
pub use ora::{decode_ora, decode_ora_with_neutral_groups};
pub use psd::decode_psd;

pub const PAINT_LOSSES: [&str; 3] = [
    "paint-layer-properties-unverified",
    "paint-canvas-fill-converted-to-layer",
    "paint-background-depth-reduced-to-8",
];

pub const PDN_LOSS: &str = "pdn-metadata-ignored";
pub const KNOWN_LOSSES: [&str; 9] = [
    PAINT_LOSSES[0],
    PAINT_LOSSES[1],
    PAINT_LOSSES[2],
    PDN_LOSS,
    crate::ora::LOSSES[0],
    crate::ora::LOSSES[1],
    crate::ora::GROUP_STRUCTURE_LOSS,
    "psd-metadata-ignored",
    "psd-empty-layer-extent-normalized",
];

#[derive(Debug)]
pub struct Converted {
    pub document: Document,
    pub losses: Vec<String>,
}

/// Observed MIF1/iovl/unci/DEFLATE layers only. Callers must acknowledge every
/// reported loss before publishing; the source never becomes a Save destination.
/// Source item labels are not qualified UI layer names. Imported layer properties
/// are deliberately visible/full opacity and numbered; this is explicit conversion.
pub fn decode_paint(
    bytes: &[u8],
    pixel_limit: u64,
    check: &dyn Fn() -> Result<()>,
) -> Result<Converted> {
    check()?;
    let limit = pixel_limit.min(4_194_304);
    let mut limits = paintcraft_paint::DecodeLimits {
        layers: 256,
        pixels: limit,
        decoded_bytes: usize::try_from(limit.saturating_mul(4)).unwrap_or(usize::MAX),
        ..Default::default()
    };
    limits.container.source_bytes = 64 << 20;
    limits.container.metadata_bytes = 4 << 20;
    let decoded = paintcraft_paint::decode_layers(bytes, &limits, check)?;
    let mut hash = Sha256::new();
    for chunk in bytes.chunks(65536) {
        check()?;
        hash.update(chunk);
    }
    let sha = paintcraft_document::hex(&hash.finalize());
    let identity = |kind: &str, object: &str| {
        deterministic_id(&["paintcraft.paint-conversion/v1", kind, &sha, object])
    };
    let mut roots = Vec::new();
    let mut layers = HashMap::new();
    roots
        .try_reserve_exact(decoded.layers.len() + 1)
        .map_err(|_| resource())?;
    layers
        .try_reserve(decoded.layers.len() + 1)
        .map_err(|_| resource())?;
    let fill = Rgba(decoded.canvas_fill.map(|sample| (sample >> 8) as u8));
    let background = identity("layer", "canvas-fill");
    roots.push(background);
    layers.insert(
        background,
        Layer {
            id: background,
            name: "Canvas background (converted)".into(),
            origin: (0, 0),
            visible: true,
            opacity: 255,
            raster: RasterLayer::new(decoded.width, decoded.height, fill),
        },
    );
    let mut admission = Admission::new(4_194_304 * 16);
    // Reserve the entire known worst-case decoded tile peak before any layer
    // ingest. Subsequent ingests consume this reservation instead of trying to
    // extend an exhausted per-operation reservation after a private prefix.
    let tiles = decoded.layers.iter().try_fold(0u64, |total, layer| {
        let count = u64::from(layer.width.div_ceil(paintcraft_raster::TILE_EDGE))
            * u64::from(layer.height.div_ceil(paintcraft_raster::TILE_EDGE));
        total.checked_add(count).ok_or_else(resource)
    })?;
    admission.reserve_tiles(tiles)?;
    for (index, source) in decoded.layers.into_iter().enumerate() {
        check()?;
        let id = identity("layer", &source.item_id.to_string());
        let raster = RasterLayer::from_rgba_checked(
            source.width,
            source.height,
            Rgba::TRANSPARENT,
            &source.rgba,
            &mut admission,
            check,
        )?;
        roots.push(id);
        layers.insert(
            id,
            Layer {
                id,
                name: format!("Imported layer {}", index + 1),
                origin: (i64::from(source.origin.0), i64::from(source.origin.1)),
                visible: true,
                opacity: 255,
                raster,
            },
        );
    }
    let mut losses = vec![PAINT_LOSSES[0].into(), PAINT_LOSSES[1].into()];
    if decoded
        .canvas_fill
        .iter()
        .any(|sample| *sample != (sample >> 8) * 257)
    {
        losses.push(PAINT_LOSSES[2].into());
    }
    check()?;
    Ok(Converted {
        document: Document {
            source_metadata: None,
            document_id: identity("document", ""),
            state_id: identity("state", ""),
            canvas: (decoded.width, decoded.height),
            roots,
            layers,
            title: None,
        },
        losses,
    })
}
fn resource() -> Error {
    Error::new(
        ErrorCode::ResourceLimit,
        "cannot allocate converted layer metadata",
    )
}

/// Convert the strict normal-blend, full-canvas PDN3 bitmap subset. Names,
/// visibility, opacity and raw RGBA are retained; source metadata and background
/// lock-role semantics require explicit conversion consent.
pub fn decode_pdn(
    bytes: &[u8],
    pixel_limit: u64,
    check: &dyn Fn() -> Result<()>,
) -> Result<Converted> {
    check()?;
    let limit = pixel_limit.min(4_194_304);
    let mut limits = paintcraft_pdn::DecodeLimits {
        graph_bytes: 4 << 20,
        graph_metadata_bytes: 8 << 20,
        decoded_pixels: limit,
        decoded_bytes: usize::try_from(limit.saturating_mul(4)).unwrap_or(usize::MAX),
        ..Default::default()
    };
    limits.envelope.source_bytes = 64 << 20;
    limits.envelope.pixels = limit;
    limits.envelope.layers = 256;
    let source = paintcraft_pdn::decode_layers(bytes, &limits, check)?;
    let mut hash = Sha256::new();
    for chunk in bytes.chunks(65536) {
        check()?;
        hash.update(chunk);
    }
    let sha = paintcraft_document::hex(&hash.finalize());
    let identity = |kind: &str, object: &str| {
        deterministic_id(&["paintcraft.pdn-conversion/v1", kind, &sha, object])
    };
    let mut roots = Vec::new();
    let mut layers = HashMap::new();
    roots
        .try_reserve_exact(source.layers.len())
        .map_err(|_| resource())?;
    layers
        .try_reserve(source.layers.len())
        .map_err(|_| resource())?;
    let tiles = source.layers.iter().try_fold(0u64, |total, layer| {
        let count = u64::from(layer.width.div_ceil(paintcraft_raster::TILE_EDGE))
            * u64::from(layer.height.div_ceil(paintcraft_raster::TILE_EDGE));
        total.checked_add(count).ok_or_else(resource)
    })?;
    let mut admission = Admission::new(4_194_304 * 16);
    admission.reserve_tiles(tiles)?;
    for layer in source.layers {
        check()?;
        if layer.name.chars().take(1025).count() > 1024 {
            return Err(Error::new(
                ErrorCode::CapabilityUnsupported,
                "PDN layer name exceeds document limits",
            ));
        }
        let mut name = String::new();
        name.try_reserve_exact(layer.name.len())
            .map_err(|_| resource())?;
        name.push_str(layer.name);
        let id = identity("layer", &layer.object_id.to_string());
        let raster = RasterLayer::from_rgba_checked(
            layer.width,
            layer.height,
            Rgba::TRANSPARENT,
            &layer.rgba,
            &mut admission,
            check,
        )?;
        roots.push(id);
        layers.insert(
            id,
            Layer {
                id,
                name,
                origin: (0, 0),
                visible: layer.visible,
                opacity: layer.opacity,
                raster,
            },
        );
    }
    check()?;
    Ok(Converted {
        document: Document {
            source_metadata: None,
            document_id: identity("document", ""),
            state_id: identity("state", ""),
            canvas: (source.width, source.height),
            roots,
            layers,
            title: None,
        },
        losses: vec![PDN_LOSS.into()],
    })
}

/// Families handled by the private layered conversion path. This is dispatch
/// information; callers must obtain frontend/operation admission separately.
pub fn supports(family: crate::registry::Family) -> bool {
    use crate::registry::Family;
    matches!(
        family,
        Family::Paint | Family::Pdn | Family::Ora | Family::Psd | Family::Psb
    )
}

pub fn decode_layered(
    family: crate::registry::Family,
    bytes: &[u8],
    pixel_limit: u64,
    check: &dyn Fn() -> Result<()>,
) -> Result<Converted> {
    match family {
        crate::registry::Family::Paint => decode_paint(bytes, pixel_limit, check),
        crate::registry::Family::Pdn => decode_pdn(bytes, pixel_limit, check),
        crate::registry::Family::Ora => decode_ora(bytes, pixel_limit, check),
        crate::registry::Family::Psd | crate::registry::Family::Psb => {
            let expected = if family == crate::registry::Family::Psd {
                1u16
            } else {
                2
            };
            if bytes.get(..4) != Some(b"8BPS") || bytes.get(4..6) != Some(&expected.to_be_bytes()) {
                return Err(Error::new(
                    ErrorCode::CodecFailed,
                    "Photoshop signature disagrees with admitted family",
                ));
            }
            decode_psd(bytes, pixel_limit, check)
        }
        _ => Err(Error::new(
            ErrorCode::CapabilityUnsupported,
            "no qualified layered conversion for this format",
        )),
    }
}
