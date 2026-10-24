//! One bounded derived viewport outline. Geometry is never selection authority.
use super::*;
use paintcraft_core::types::{Result, resources::SharedBudget};
use std::sync::{Arc, OnceLock};
#[derive(Debug)]
struct Entry {
    mask: BinaryMask,
    clip: Rect,
    activity: u64,
    result: Result<Arc<MaskOutline>>,
}
#[derive(Debug)]
pub struct OutlineCache {
    entry: Option<Entry>,
    budget: SharedBudget,
}
impl Default for OutlineCache {
    fn default() -> Self {
        static REALM: OnceLock<SharedBudget> = OnceLock::new();
        Self::with_budget(REALM.get_or_init(|| SharedBudget::new(16 << 20)).clone())
    }
}
impl OutlineCache {
    pub fn with_budget(budget: SharedBudget) -> Self {
        Self {
            entry: None,
            budget,
        }
    }
    pub fn clear(&mut self) {
        self.entry = None;
    }
    /// Cache success and failure for an unchanged field/viewport. Resource
    /// activity invalidates only failures, so released shared capacity can retry.
    pub fn get(&mut self, mask: &BinaryMask, clip: Rect) -> Result<Arc<MaskOutline>> {
        if let Some(entry) = &self.entry
            && entry.mask == *mask
            && entry.clip == clip
            && (entry.result.is_ok() || entry.activity == self.budget.activity())
        {
            return entry.result.clone();
        }
        // Drop the old cache before attempting replacement; external readers
        // keep their own resource ownership until finished.
        self.clear();
        let result = mask
            .outline_checked(
                clip,
                &self.budget,
                OutlineLimits {
                    work: 2_097_152,
                    segments: 16_384,
                },
                &|| Ok(()),
            )
            .map(Arc::new);
        self.entry = Some(Entry {
            mask: mask.clone(),
            clip,
            activity: self.budget.activity(),
            result: result.clone(),
        });
        result
    }
}
