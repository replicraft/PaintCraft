//! Pure defining control facts for a separately admitted source forecast.
use super::*;
use paintcraft_document::{FilterInstance, FilterMask, LayerMask};
use paintcraft_raster::{PortableLayerConfig, portable_pixel_geometry_bytes};

/// Constructor control declarations only, in their respective ORIGINAL roles.
/// No source, payload, factory, account or execution authority is returned.
/// The existing Plan/scan quote and source probe transient quote remain separate.
#[repr(C)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct PortableSource8MaterializerControlBytes {
    /// Coverage wrapper retained in A while the document stream is live.
    pub coverage: u64,
    /// Document-intention stream, additional to the live coverage wrapper in A.
    pub document_stream: u64,
    /// Pixel wrapper in C, additional to the Raster builder and decoder scratch.
    pub pixels: u64,
    /// Retained-layout census control in P, additional to Plan/records/helpers.
    pub retained_layout: u64,
}

/// Pure target-layout facts; creates no Plan, source, allocation or grant.
///
/// The returned32-byte facts and their Result/movement controls belong to the
/// embedding caller. No callbacks, locks, source parsing, output allocation or
/// budget admission occurs. These quantities do not include retained payloads,
/// canonical A8 control4096, pixel-builder4096, decoder controls/scratch, factory
/// controls, allocator overhead/slack or RSS. Existing constructor work and
/// admission behavior are unchanged; the real forecast must prepay its query
/// and combine the actual simultaneous P/A/C phases before requesting storage.
/// Empty PRESENT masks still need canonical A8 control, even with no StorageArc.
/// Pixel wrappers are needed on every layer when GLOBAL tiles>0; only globalT0
/// skips the decoder/pixel branch in the current closed reader.
pub fn portable_source8_materializer_control_bytes()
-> Result<PortableSource8MaterializerControlBytes> {
    Ok(PortableSource8MaterializerControlBytes {
        coverage: coverage_bytes()?,
        document_stream: stream_bytes()?,
        pixels: pixel_bytes()?,
        retained_layout: retained_layout::control_bytes()?,
    })
}
// Exact ordered defining terms of coverage::coverage_materializer's inline sum.
fn coverage_bytes() -> Result<u64> {
    let bytes = [
        size_of::<PortableCoverageMaterializer8<'static, 'static>>(),
        size_of::<wire::Cursor<'static, 'static>>(),
        size_of::<PortableMaskView<'static>>(),
        size_of::<PortableLayerCursor<'static, 'static>>(),
        4 * size_of::<u64>(),
        2 * size_of::<usize>(),
    ]
    .into_iter()
    .try_fold(0usize, |n, v| n.checked_add(v).ok_or_else(bound))?;
    Ok(bytes as u64)
}
// Exact ordered defining terms of coverage::stream::stream_control_bytes.
fn stream_bytes() -> Result<u64> {
    [
        size_of::<DocumentIntentions8<'static, 'static, 'static>>(),
        size_of::<LayerIntentions8<'static, 'static, 'static>>(),
        size_of::<PortableLayerHeader<'static>>(),
        size_of::<PortableFilterView<'static>>(),
        2 * size_of::<FilterInstance>(),
        size_of::<FilterMask>(),
        size_of::<LayerMask>(),
        size_of::<PortableMaskView<'static>>(),
        size_of::<wire::Cursor<'static, 'static>>(),
        size_of::<LayerRecord>(),
        size_of::<IntentionReceipt8>(),
        size_of::<paintcraft_raster::coverage_mask::Limits>(),
        2 * size_of::<paintcraft_core::types::filters::Descriptor>(),
        2 * size_of::<uuid::Uuid>(),
        size_of::<[u64; 8]>(),
        size_of::<[usize; 8]>(),
    ]
    .into_iter()
    .try_fold(0u64, |n, v| add(n, v as u64))
}
// Exact ordered defining terms of pixels::pixel_materializer's inline sum.
fn pixel_bytes() -> Result<u64> {
    let bytes = [
        size_of::<PortablePixelMaterializer8<'static, 'static>>(),
        size_of::<PortableLayerConfig>(),
        size_of::<wire::Cursor<'static, 'static>>(),
        size_of::<PortableLayerHeader<'static>>(),
        size_of::<PortableLayerCursor<'static, 'static>>(),
        size_of::<PortableBlockView<'static>>(),
        2 * size_of::<usize>(),
        usize::try_from(portable_pixel_geometry_bytes()).map_err(|_| bound())?,
        size_of::<[u64; 8]>(),
        4 * size_of::<usize>(),
    ]
    .into_iter()
    .try_fold(0usize, |n, v| n.checked_add(v).ok_or_else(bound))?;
    Ok(bytes as u64)
}

#[cfg(test)]
#[path = "materializer_controls/tests.rs"]
mod tests;
