//! Explicit compound consent for styled PNG copies. This is independent of the
//! legacy filter-only policy and never permits preserving or foreign codecs.
use super::metadata_policy::Target;
use crate::registry::{Family, Operation};
use paintcraft_core::types::{Error, ErrorCode, Result, json::Value, policy::hex};
use paintcraft_document::{Document, LAYER_COUNT_MAX};
use sha2::{Digest, Sha256};
use std::collections::HashSet;
use uuid::Uuid;
pub const BAKED_REASON: &str = "styles-baked";
pub const COMPOUND_REASON: &str = "layer-appearance-baked";
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Intent {
    #[default]
    Refuse,
    Bake,
}
impl std::str::FromStr for Intent {
    type Err = Error;
    fn from_str(s: &str) -> Result<Self> {
        match s {
            "refuse" => Ok(Self::Refuse),
            "bake" => Ok(Self::Bake),
            _ => Err(invalid("Appearance copy policy must be refuse or bake")),
        }
    }
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Inventory {
    digest: [u8; 32],
    styles: u64,
    filters: u64,
    filter_masks: u64,
    layer_masks: u64,
    non_normal: u64,
}
impl Inventory {
    pub fn digest(self) -> [u8; 32] {
        self.digest
    }
    pub fn style_count(self) -> u64 {
        self.styles
    }
    pub fn filter_count(self) -> u64 {
        self.filters
    }
    pub fn filter_mask_count(self) -> u64 {
        self.filter_masks
    }
    pub fn layer_mask_count(self) -> u64 {
        self.layer_masks
    }
    pub fn non_normal_layer_count(self) -> u64 {
        self.non_normal
    }
    pub fn has_styles(self) -> bool {
        self.styles != 0
    }
    pub fn has_filters(self) -> bool {
        self.filters != 0
    }
    pub fn has_filter_masks(self) -> bool {
        self.filter_masks != 0
    }
    pub fn has_layer_masks(self) -> bool {
        self.layer_masks != 0
    }
    pub fn has_layer_blends(self) -> bool {
        self.non_normal != 0
    }
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct BakeWitness {
    document: Uuid,
    state: Uuid,
    inventory: Inventory,
    target: Target,
}
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Policy {
    #[default]
    Refuse,
    Bake(BakeWitness),
}
fn invalid(s: &str) -> Error {
    Error::new(ErrorCode::InputSchema, s)
}
fn limit() -> Error {
    Error::new(
        ErrorCode::ResourceLimit,
        "Compound appearance inventory limit",
    )
}
fn require_target(t: Target) -> Result<()> {
    if t.family == Family::Png
        && matches!(t.operation, Operation::ExportFlat | Operation::SaveAsCopy)
    {
        Ok(())
    } else {
        Err(Error::new(
            ErrorCode::CapabilityUnsupported,
            "Compound appearance baking requires an explicit PNG export copy",
        ))
    }
}
fn add(count: &mut u64, n: u64) -> Result<()> {
    *count = count.checked_add(n).ok_or_else(limit)?;
    Ok(())
}
/// Bounded snapshot metadata inventory, not a render admission or consent. The
/// duplicate-ID scratch is fallibly reserved under the existing layer bound;
/// this inherited export-boundary convention is not whole-process accounting.
/// Raster content uses the host's immutable document/state contract, not a new
/// pixel walk. Disabled, hidden and zero-opacity intentions remain inventoried.
/// # Errors
/// Propagates cancellation and refuses malformed roots or allocation overflow.
pub fn inspect(doc: &Document, check: &dyn Fn() -> Result<()>) -> Result<Inventory> {
    check()?;
    if doc.roots.len() > LAYER_COUNT_MAX || doc.layers.len() > LAYER_COUNT_MAX {
        return Err(limit());
    }
    if doc.roots.len() != doc.layers.len() {
        return Err(invalid("Appearance roots differ from layer map"));
    }
    let mut seen = HashSet::new();
    seen.try_reserve(doc.roots.len()).map_err(|_| limit())?;
    let mut hash = Sha256::new();
    hash.update(b"paintcraft.compound-png-appearance/v1\0");
    hash.update(doc.document_id.as_bytes());
    hash.update(doc.state_id.as_bytes());
    hash.update(doc.canvas.0.to_be_bytes());
    hash.update(doc.canvas.1.to_be_bytes());
    hash.update((doc.roots.len() as u64).to_be_bytes());
    let mut out = Inventory {
        digest: [0; 32],
        styles: 0,
        filters: 0,
        filter_masks: 0,
        layer_masks: 0,
        non_normal: 0,
    };
    for id in &doc.roots {
        check()?;
        if !seen.insert(*id) {
            return Err(invalid("Appearance roots repeat a layer"));
        }
        let l = doc
            .layers
            .get(id)
            .ok_or_else(|| invalid("Appearance layer is missing"))?;
        if l.id != *id {
            return Err(invalid("Appearance layer identity differs"));
        }
        add(&mut out.styles, l.styles.len() as u64)?;
        add(&mut out.filters, l.filters.len() as u64)?;
        for f in l.filters.iter() {
            check()?;
            if f.mask().is_some() {
                add(&mut out.filter_masks, 1)?
            }
        }
        if l.mask.is_some() {
            add(&mut out.layer_masks, 1)?
        }
        if l.blend_mode != paintcraft_core::types::LayerBlendMode::Normal {
            add(&mut out.non_normal, 1)?
        }
        hash.update(id.as_bytes());
        hash.update((l.name.len() as u64).to_be_bytes());
        hash.update(l.name.as_bytes());
        hash.update([u8::from(l.visible), l.opacity, l.blend_mode as u8]);
        hash.update(l.origin.0.to_be_bytes());
        hash.update(l.origin.1.to_be_bytes());
        hash.update(l.raster.width().to_be_bytes());
        hash.update(l.raster.height().to_be_bytes());
        hash.update(l.raster.default_pixel().0);
        hash.update(l.filters.digest());
        hash.update(l.styles.digest());
        match &l.mask {
            None => hash.update([0]),
            Some(mask) => {
                hash.update([1]);
                hash.update(mask.digest());
            }
        }
    }
    check()?;
    out.digest = hash.finalize().into();
    Ok(out)
}
impl Policy {
    /// Capture the actual frozen source and explicit PNG target, not the live UI.
    /// # Errors
    /// Refuses unsupported targets or a Bake intent without stored styles.
    pub fn capture(
        doc: &Document,
        target: Target,
        intent: Intent,
        check: &dyn Fn() -> Result<()>,
    ) -> Result<Self> {
        check()?;
        if intent == Intent::Refuse {
            return Ok(Self::Refuse);
        }
        require_target(target)?;
        let inventory = inspect(doc, check)?;
        if !inventory.has_styles() {
            return Err(invalid(
                "Compound appearance consent requires stored styles",
            ));
        }
        Ok(Self::Bake(BakeWitness {
            document: doc.document_id,
            state: doc.state_id,
            inventory,
            target,
        }))
    }
    /// Backend-supplied target must match; old loss flags cannot mint authority.
    /// # Errors
    /// Refuses absent consent, changed inventory/source or another output route.
    pub fn validate(
        self,
        doc: &Document,
        target: Target,
        check: &dyn Fn() -> Result<()>,
    ) -> Result<bool> {
        check()?;
        match self {
            Self::Refuse => {
                doc.require_no_styles()?;
                Ok(false)
            }
            Self::Bake(w) => {
                require_target(target)?;
                let current = inspect(doc, check)?;
                if w.document != doc.document_id
                    || w.state != doc.state_id
                    || w.target != target
                    || w.inventory != current
                {
                    return Err(Error::new(
                        ErrorCode::ConflictStaleBase,
                        "Appearance bake witness does not match this frozen document and target",
                    ));
                }
                Ok(true)
            }
        }
    }
    /// Actual successful export disclosure. Call only after validate and encode;
    /// this value describes an outcome and cannot be used to authorize baking.
    pub(crate) fn disclosure(self) -> Option<Value> {
        let Self::Bake(w) = self else { return None };
        let i = w.inventory;
        Some(Value::object([
            (
                "schema",
                Value::str("paintcraft.export-disclosure/layer-styles-v1"),
            ),
            ("document_id", Value::str(w.document.to_string())),
            ("source_state_id", Value::str(w.state.to_string())),
            (
                "target",
                Value::object([
                    ("family", Value::str("png")),
                    (
                        "operation",
                        Value::str(match w.target.operation {
                            Operation::SaveAsCopy => "save-as-copy",
                            _ => "export-flat",
                        }),
                    ),
                ]),
            ),
            ("inventory_sha256", Value::str(hex(&i.digest))),
            (
                "counts",
                Value::object([
                    ("styles", Value::Int(i.styles as i64)),
                    ("filters", Value::Int(i.filters as i64)),
                    ("filter_masks", Value::Int(i.filter_masks as i64)),
                    ("layer_masks", Value::Int(i.layer_masks as i64)),
                    ("non_normal_layers", Value::Int(i.non_normal as i64)),
                ]),
            ),
            ("styles_baked", Value::Bool(i.has_styles())),
            ("filters_baked", Value::Bool(i.has_filters())),
            ("filter_masks_baked", Value::Bool(i.has_filter_masks())),
            ("layer_masks_baked", Value::Bool(i.has_layer_masks())),
            ("layer_blends_baked", Value::Bool(i.has_layer_blends())),
        ]))
    }
}
#[cfg(test)]
mod tests;
