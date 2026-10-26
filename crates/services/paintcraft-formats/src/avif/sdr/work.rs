//! Explicit large-buffer capacities, inside the existing combined Rust/C child cap.
use super::*;
use paintcraft_core::types::{Error, ErrorCode};
/// Large native/reconstruction buffers are declared separately so i32 samples
/// are never mistaken for byte samples. The fixed child heap remains the actual
/// requested-allocation cap, covering metadata/framework allocations as well.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct WorkLayout {
    pub source_bytes: u64,
    pub native_planes_bytes: u64,
    pub staged_samples_bytes: u64,
    pub rgba_bytes: u64,
    pub converter_scratch_bytes: u64,
    pub reconstruction_phase_bytes: u64,
    pub publication_phase_bytes: u64,
    /// Includes the entire combined Rust+C child cap and separate host source,
    /// output and verification buffers. Raster buffers above are inside that
    /// child cap, not another allocator charge on top of it.
    pub host_reservation_bytes: u64,
}
impl WorkLayout {
    pub fn require_heap(&self, bytes: u64) -> Result<()> {
        if self
            .reconstruction_phase_bytes
            .max(self.publication_phase_bytes)
            > bytes
        {
            return Err(Error::new(
                ErrorCode::ResourceLimit,
                "AVIF SDR declared buffers exceed child allowance",
            ));
        }
        Ok(())
    }
}
/// Declare capacities before native decode or raster staging. Valid only for
/// the selected HEIF color converter and bounded AVIF candidate geometry.
/// Axis storage includes alignment; row storage conservatively covers every
/// 16-row band even though the actual helper uses one Rayon worker. Stack,
/// allocator overhead and physical RSS are not represented by these fields.
pub fn work_layout(admitted: &Admitted<'_>, source_bytes: u64) -> Result<WorkLayout> {
    // Reject externally supplied lengths before any phase arithmetic.
    let host_reservation_bytes = crate::codec_worker::reservation_bytes_for_input(source_bytes)?;
    let primary = &admitted.primary;
    let coded = u64::try_from(primary.byte_len)
        .ok()
        .and_then(|bytes| {
            u64::try_from(admitted.alpha.as_ref().map_or(0, |value| value.byte_len))
                .ok()
                .and_then(|alpha| bytes.checked_add(alpha))
        })
        .ok_or_else(|| {
            Error::new(
                ErrorCode::ResourceLimit,
                "AVIF SDR coded byte count overflow",
            )
        })?;
    let count = u64::from(primary.width) * u64::from(primary.height);
    if count == 0
        || count > crate::avif::INSPECT_PIXELS_MAX
        || primary.width > 65536
        || primary.height > 65536
        || primary.byte_len == 0
        || admitted
            .alpha
            .as_ref()
            .is_some_and(|value| value.byte_len == 0)
        || source_bytes < coded
    {
        return Err(Error::new(
            ErrorCode::ResourceLimit,
            "AVIF SDR work geometry/source allowance",
        ));
    }
    let width = u64::from(primary.width);
    let height = u64::from(primary.height);
    let sub = if primary.sequence.subsampling_x && primary.sequence.subsampling_y {
        2
    } else {
        1
    };
    let chroma = width.div_ceil(sub) * height.div_ceil(sub);
    let alpha = u64::from(admitted.alpha.is_some());
    let native_planes_bytes = count * 3 * (1 + alpha);
    let staged_samples_bytes = (count + chroma * 2 + count * alpha) * size_of::<i32>() as u64;
    let rgba_bytes = count * 4;
    // Private Axis has two usize indices and one f32 plus alignment padding.
    let axis = (width + height) * 3 * size_of::<usize>() as u64;
    let rows = width * 8 * size_of::<f32>() as u64 * height.div_ceil(16);
    let converter_scratch_bytes = axis + rows + height;
    let reconstruction_phase_bytes = source_bytes
        + native_planes_bytes
        + staged_samples_bytes
        + rgba_bytes
        + converter_scratch_bytes;
    // Native/i32 planes have retired before the response duplicates RGBA.
    let publication_phase_bytes =
        source_bytes + rgba_bytes * 2 + crate::codec_worker::RESPONSE_HEADER as u64;
    let layout = WorkLayout {
        source_bytes,
        native_planes_bytes,
        staged_samples_bytes,
        rgba_bytes,
        converter_scratch_bytes,
        reconstruction_phase_bytes,
        publication_phase_bytes,
        host_reservation_bytes,
    };
    layout.require_heap(crate::codec_worker::HEAP_BYTES)?;
    Ok(layout)
}
