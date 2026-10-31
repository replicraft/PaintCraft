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
    /// Present only for the separately negotiated catalog source4 profile.
    pub catalog_count: Option<u64>,
}
impl SpatialProfile {
    pub fn parse(value: &Value, layers: usize) -> Result<Option<Self>> {
        if value.get("protocol").is_none() {
            return Ok(None);
        }
        let protocol = integer(field(value, "protocol")?)?;
        if !matches!(protocol, 3 | 4) {
            return Err(bad("Unknown filter source protocol"));
        }
        let count = integer(field(value, "spatialCount")?)?;
        let bytes = integer(field(value, "workspaceBytes")?)?;
        let work = integer(field(value, "workLimit")?)?;
        let catalog_count = (protocol == 4)
            .then(|| integer(field(value, "catalogCount")?))
            .transpose()?;
        if count > layers as u64 * 256
            || catalog_count.is_some_and(|count| count > layers as u64 * 256)
            || bytes > WORKSPACE_BYTES_MAX
            || work > WORK_MAX
        {
            return Err(resource("Spatial source workspace declaration"));
        }
        Ok(Some(Self {
            count,
            bytes,
            work,
            catalog_count,
        }))
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
pub fn stored_catalog(doc: &paintcraft_document::Document) -> u64 {
    doc.layers
        .values()
        .map(|layer| {
            layer
                .filters
                .iter()
                .filter(|filter| filter.descriptor().is_catalog_only())
                .count() as u64
        })
        .sum()
}
/// Source4's initial prepared capability remains closed even if a later build
/// teaches the generic PCND6 reader about more numerical profiles.
pub fn catalog_kind_allowed(kind: &str) -> bool {
    matches!(
        kind,
        "invert-rgb-v1"
            | "brightness-contrast-v1"
            | "black-white-rgb-v1"
            | "threshold-luma-v1"
            | "posterize-rgb-v1"
            | "solarize-rgb-v1"
            | "box-blur-v1"
            | "levels-rgb-v1"
    )
}
pub fn refused() -> Error {
    Error::new(
        ErrorCode::CapabilityUnsupported,
        "Spatial filters require source protocol3 and explicit workspace admission",
    )
}
