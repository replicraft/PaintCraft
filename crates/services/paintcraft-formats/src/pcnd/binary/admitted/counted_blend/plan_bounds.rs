//! Paid sizing facts from the same immutable source as the complete count.
use super::*;

const QUERY_CONTROL: u64 = 768;
const ROOT_WORK: u64 = 2048;

/// Record counts for sizing a subsequent authenticated Source8 plan.
///
/// These are data bounds, not memory grants or validation of encoded bytes.
/// Plan controls, scan/census scratch and source-probe peaks remain additional.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct BlendPlanRecordUpperBounds {
    /// Exact layer count of the immutable counted source.
    pub layers: u64,
    /// Upper bound on serialized tile records; all-default tiles may be omitted.
    pub tiles: u64,
}

impl CountedBlendSnapshot<'_> {
    /// Bound plan records using the original source, scope and callbacks.
    ///
    /// The successful complete count already validated flat root identities.
    /// This separate paid walk looks up those roots and reads each raster's
    /// constant-time resident count. The writer emits at most one record per
    /// resident tile. No pixels or HashMap buckets are scanned, and table
    /// capacity is never used as a count or a grant.
    ///
    /// Temporary query controls use 768 bytes of the original scope. The
    /// returned 16-byte facts belong to the caller's control declaration.
    /// The subsequent plan must authenticate the actual encoded bytes and
    /// account for its simultaneous controls and scratch separately.
    ///
    /// # Errors
    /// Returns the original callback error, ResourceLimit for exhausted work,
    /// query storage or arithmetic, or InvariantViolation for changed roots.
    pub fn plan_record_upper_bounds_checked(&self) -> Result<BlendPlanRecordUpperBounds> {
        (self.spend)(64)?;
        let named = 2 * size_of::<Result<BlendPlanRecordUpperBounds>>()
            + size_of::<Error>()
            + size_of::<BudgetAccount>()
            + size_of::<BlendPlanRecordUpperBounds>()
            + 5 * size_of::<u64>()
            + 3 * size_of::<usize>()
            + 4 * size_of::<&dyn Fn() -> Result<()>>();
        if named as u64 > QUERY_CONTROL || size_of::<BlendPlanRecordUpperBounds>() != 16 {
            return Err(limit("Source8 plan-bound query controls"));
        }
        (self.spend)(2 * QUERY_CONTROL + 128)?;
        let mut control = self.scope.account();
        control.resize(QUERY_CONTROL)?;
        step(self.check, self.spend)?;
        let mut bounds = BlendPlanRecordUpperBounds {
            layers: self.doc.roots.len() as u64,
            tiles: 0,
        };
        for id in &self.doc.roots {
            // Root lookup policy plus checked count/identity scalar work.
            (self.spend)(ROOT_WORK + 64)?;
            step(self.check, self.spend)?;
            let layer = self
                .doc
                .layers
                .get(id)
                .filter(|layer| layer.id == *id)
                .ok_or_else(mismatch)?;
            bounds.tiles = bounds
                .tiles
                .checked_add(layer.raster.resident_tiles() as u64)
                .ok_or_else(|| limit("Source8 plan record count overflow"))?;
        }
        step(self.check, self.spend)?;
        Ok(bounds)
    }
}

#[cfg(test)]
mod tests;
