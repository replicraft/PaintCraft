//! Source profile3 adds explicit stored-spatial facts and a fixed workspace domain.
use super::*;
pub const WORKSPACE_BYTES_MAX: u64 = 64 << 20;
pub const WORK_MAX: u64 = 268_435_456;
pub const NATIVE_EDGE: i64 = 64;
pub const PATCH_EDGE: u32 = 32;
pub const NATIVE_STEP_WORK_MAX: u64 = 8_388_608;
#[derive(Clone, Copy, Debug)]
pub struct SpatialProfile {
    pub count: u64,
    pub bytes: u64,
    pub work: u64,
}
impl SpatialProfile {
    pub fn parse(value: &Value, layers: usize) -> Result<Option<Self>> {
        if value.get("protocol").is_none() {
            return Ok(None);
        }
        if integer(field(value, "protocol")?)? != 3 {
            return Err(bad("Unknown filter source protocol"));
        }
        let count = integer(field(value, "spatialCount")?)?;
        let bytes = integer(field(value, "workspaceBytes")?)?;
        let work = integer(field(value, "workLimit")?)?;
        if count > layers as u64 * 256 || bytes > WORKSPACE_BYTES_MAX || work > WORK_MAX {
            return Err(resource("Spatial source workspace declaration"));
        }
        Ok(Some(Self { count, bytes, work }))
    }
    pub fn extra_quote(self) -> Result<u64> {
        self.bytes
            .checked_add(
                1024 * std::mem::size_of::<paintcraft_render::display::DisplayAccumulator>() as u64,
            )
            .and_then(|n| n.checked_add((NATIVE_EDGE * NATIVE_EDGE * 4) as u64))
            .ok_or_else(|| resource("Spatial source workspace overflow"))
    }
}
pub fn stored(doc: &paintcraft_document::Document) -> u64 {
    doc.layers
        .values()
        .map(|layer| {
            layer
                .filters
                .iter()
                .filter(|filter| filter.descriptor().is_spatial())
                .count() as u64
        })
        .sum()
}
pub fn refused() -> Error {
    Error::new(
        ErrorCode::CapabilityUnsupported,
        "Spatial filters require source protocol3 and explicit workspace admission",
    )
}
