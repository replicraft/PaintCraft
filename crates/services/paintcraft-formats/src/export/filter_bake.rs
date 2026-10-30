//! Explicit authority to discard editable filter intentions in one foreign copy.
//! This does not authorize a codec, metadata omission or source-pixel mutation.
//! Visual consumers (display, printing, clipboard, picker and thumbnails) are not
//! editable-file copies and must not be gated by this policy.
use super::metadata_policy::Target;
use paintcraft_core::types::{Error, ErrorCode, Result};
use paintcraft_document::{Document, LAYER_COUNT_MAX};
use sha2::{Digest, Sha256};
use std::collections::HashSet;
use uuid::Uuid;

pub const BAKED_REASON: &str = "filters-baked";
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Intent {
    #[default]
    Refuse,
    Bake,
}
impl std::str::FromStr for Intent {
    type Err = Error;
    fn from_str(value: &str) -> Result<Self> {
        match value {
            "refuse" => Ok(Self::Refuse),
            "bake" => Ok(Self::Bake),
            _ => Err(Error::new(
                ErrorCode::InputSchema,
                "Filter copy policy must be refuse or bake",
            )),
        }
    }
}

/// Only capture can mint this witness. IDs assume the host's immutable snapshot
/// contract; this metadata fingerprint deliberately does not hash raster pixels.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct BakeWitness {
    document: Uuid,
    state: Uuid,
    inventory: [u8; 32],
    target: Target,
    has_filters: bool,
}
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Policy {
    #[default]
    Refuse,
    Bake(BakeWitness),
}
fn require_copy(target: Target) -> Result<()> {
    if target.is_foreign_copy() {
        Ok(())
    } else {
        Err(Error::new(
            ErrorCode::InputSchema,
            "Filter baking requires an explicit foreign export copy; PCND and preserving operations retain filters",
        ))
    }
}
/// Read-only facts about one exact candidate, not resource admission or consent.
/// Storage counts retained filter allocations, not raster, scratch or GPU memory.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Inventory {
    digest: [u8; 32],
    has_filters: bool,
    filter_count: u64,
    filter_storage_bytes: u64,
}
impl Inventory {
    pub fn digest(self) -> [u8; 32] {
        self.digest
    }
    pub fn has_filters(self) -> bool {
        self.has_filters
    }
    pub fn filter_count(self) -> u64 {
        self.filter_count
    }
    pub fn filter_storage_bytes(self) -> u64 {
        self.filter_storage_bytes
    }
}
fn invalid(message: &str) -> Error {
    Error::new(ErrorCode::InputSchema, message)
}

/// Linear metadata walk at freeze/backend validation boundaries, not per frame.
/// The fallibly reserved duplicate-ID set is bounded by the existing document
/// layer cap. This bounded scratch is not a total process-memory/RSS guarantee.
pub fn inspect(doc: &Document, check: &dyn Fn() -> Result<()>) -> Result<Inventory> {
    check()?;
    if doc.roots.len() > LAYER_COUNT_MAX || doc.layers.len() > LAYER_COUNT_MAX {
        return Err(Error::new(
            ErrorCode::ResourceLimit,
            "Filter copy layer inventory exceeds document limit",
        ));
    }
    if doc.roots.len() != doc.layers.len() {
        return Err(invalid("Filter copy root inventory differs from layer map"));
    }
    let mut seen = HashSet::new();
    check()?;
    seen.try_reserve(doc.roots.len()).map_err(|_| {
        Error::new(
            ErrorCode::ResourceLimit,
            "Cannot allocate filter copy inventory scratch",
        )
    })?;
    let mut digest = Sha256::new();
    digest.update(b"paintcraft.filter-bake-witness/v1\0");
    digest.update(doc.document_id.as_bytes());
    digest.update(doc.state_id.as_bytes());
    digest.update(doc.canvas.0.to_be_bytes());
    digest.update(doc.canvas.1.to_be_bytes());
    digest.update((doc.roots.len() as u64).to_be_bytes());
    let mut has_filters = false;
    let mut filter_count = 0_u64;
    let mut filter_storage_bytes = 0_u64;
    for id in &doc.roots {
        check()?;
        if !seen.insert(*id) {
            return Err(invalid("Filter copy root inventory repeats a layer"));
        }
        let layer = doc
            .layers
            .get(id)
            .ok_or_else(|| invalid("Filter copy root layer is missing"))?;
        if layer.id != *id {
            return Err(invalid("Filter copy layer key differs from its identity"));
        }
        has_filters |= !layer.filters.is_empty();
        filter_count = filter_count
            .checked_add(layer.filters.len() as u64)
            .ok_or_else(|| {
                Error::new(ErrorCode::ResourceLimit, "Filter inventory count overflow")
            })?;
        let storage = u64::try_from(layer.filters.storage_bytes()).map_err(|_| {
            Error::new(
                ErrorCode::ResourceLimit,
                "Filter storage exceeds inventory address range",
            )
        })?;
        filter_storage_bytes = filter_storage_bytes.checked_add(storage).ok_or_else(|| {
            Error::new(
                ErrorCode::ResourceLimit,
                "Filter inventory storage overflow",
            )
        })?;
        digest.update(id.as_bytes());
        digest.update(layer.filters.digest());
        digest.update([u8::from(layer.visible), layer.opacity]);
        digest.update(layer.origin.0.to_be_bytes());
        digest.update(layer.origin.1.to_be_bytes());
        digest.update(layer.raster.width().to_be_bytes());
        digest.update(layer.raster.height().to_be_bytes());
        digest.update(layer.raster.default_pixel().0);
    }
    check()?;
    Ok(Inventory {
        digest: digest.finalize().into(),
        has_filters,
        filter_count,
        filter_storage_bytes,
    })
}
impl Policy {
    /// Capture against the actual frozen export snapshot, including private text
    /// or brush pixels. Owner/session freshness remains the host's responsibility.
    pub fn capture(
        doc: &Document,
        target: Target,
        intent: Intent,
        check: &dyn Fn() -> Result<()>,
    ) -> Result<Self> {
        check()?;
        if intent == Intent::Bake {
            require_copy(target)?;
        }
        let inventory = inspect(doc, check)?;
        match intent {
            Intent::Refuse => Ok(Self::Refuse),
            Intent::Bake => Ok(Self::Bake(BakeWitness {
                document: doc.document_id,
                state: doc.state_id,
                inventory: inventory.digest,
                target,
                has_filters: inventory.has_filters,
            })),
        }
    }
    /// The backend supplies its own hardcoded target. Return true only when
    /// editable intentions are actually omitted, including disabled/hidden ones.
    /// Loss-name acknowledgment and metadata Omit cannot construct this policy.
    pub fn validate(
        self,
        doc: &Document,
        expected: Target,
        check: &dyn Fn() -> Result<()>,
    ) -> Result<bool> {
        check()?;
        if matches!(self, Self::Bake(_)) {
            require_copy(expected)?;
        }
        let inventory = inspect(doc, check)?;
        match self {
            Self::Refuse => {
                if inventory.has_filters {
                    return Err(Error::new(
                        ErrorCode::CapabilityUnsupported,
                        "Export requires explicit consent to bake editable filters into a copy",
                    ));
                }
                Ok(false)
            }
            Self::Bake(witness) => {
                if witness.document != doc.document_id
                    || witness.state != doc.state_id
                    || witness.target != expected
                    || witness.inventory != inventory.digest
                    || witness.has_filters != inventory.has_filters
                {
                    return Err(Error::new(
                        ErrorCode::ConflictStaleBase,
                        "Filter bake witness does not match this frozen document and export target",
                    ));
                }
                Ok(inventory.has_filters)
            }
        }
    }
}

#[cfg(test)]
mod tests;
