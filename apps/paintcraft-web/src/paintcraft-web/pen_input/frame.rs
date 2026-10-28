use super::*;
/// Source-derived descriptors use State's fixed quote. Additional framework
/// descriptors are admitted before this new wrapper Vec is allocated.
pub(crate) struct CanvasFrame {
    events: Vec<CanvasEvent>,
    charge: BudgetAccount,
}
pub(crate) struct CanvasIter {
    events: std::vec::IntoIter<CanvasEvent>,
    _charge: BudgetAccount,
}
impl CanvasFrame {
    pub(super) fn new(
        capacity: usize,
        framework: usize,
        budget: &SharedBudget,
    ) -> paintcraft_core::types::Result<Self> {
        let mut charge = budget.account(AllocationClass::PaintMetadata);
        let bytes = framework
            .checked_mul(std::mem::size_of::<CanvasEvent>())
            .ok_or_else(|| {
                paintcraft_core::types::Error::new(
                    paintcraft_core::types::ErrorCode::ResourceLimit,
                    "Canvas input descriptor overflow",
                )
            })?;
        charge.resize(bytes as u64)?;
        let mut events = Vec::new();
        events.try_reserve_exact(capacity).map_err(|_| {
            paintcraft_core::types::Error::new(
                paintcraft_core::types::ErrorCode::ResourceLimit,
                "Canvas input descriptors unavailable",
            )
        })?;
        // try_reserve_exact may return greater capacity. The fixed source quote
        // covers only requested source slots; reconcile any known extra slots.
        let extra = events.capacity().checked_sub(capacity).ok_or_else(|| {
            paintcraft_core::types::Error::new(
                paintcraft_core::types::ErrorCode::InvariantViolation,
                "Canvas descriptor capacity is smaller than admitted",
            )
        })?;
        let actual = extra
            .checked_mul(std::mem::size_of::<CanvasEvent>())
            .and_then(|extra| bytes.checked_add(extra))
            .ok_or_else(|| {
                paintcraft_core::types::Error::new(
                    paintcraft_core::types::ErrorCode::ResourceLimit,
                    "Canvas descriptor capacity overflow",
                )
            })?;
        charge.resize(actual as u64)?;
        Ok(Self { events, charge })
    }
    pub(super) fn empty(budget: &SharedBudget) -> Self {
        Self {
            events: Vec::new(),
            charge: budget.account(AllocationClass::PaintMetadata),
        }
    }
    pub(super) fn push(&mut self, event: CanvasEvent) {
        self.events.push(event);
    }
    pub(super) fn extend(&mut self, events: impl Iterator<Item = CanvasEvent>) {
        self.events.extend(events);
    }
}
impl IntoIterator for CanvasFrame {
    type Item = CanvasEvent;
    type IntoIter = CanvasIter;
    fn into_iter(self) -> CanvasIter {
        CanvasIter {
            events: self.events.into_iter(),
            _charge: self.charge,
        }
    }
}
impl Iterator for CanvasIter {
    type Item = CanvasEvent;
    fn next(&mut self) -> Option<CanvasEvent> {
        self.events.next()
    }
}
