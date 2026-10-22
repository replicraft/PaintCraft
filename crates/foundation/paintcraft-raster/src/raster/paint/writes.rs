//! Shared immutable descriptor storage; writes acquire independent charged copies.
use super::*;
use paintcraft_core::types::resources::{AllocationClass, BudgetAccount};

#[derive(Debug)]
pub(super) struct Writes {
    // Storage must drop before its declaration, including during unwind.
    pub values: Vec<Write>,
    pub charge: BudgetAccount,
}
impl std::ops::Deref for Writes {
    type Target = Vec<Write>;
    fn deref(&self) -> &Self::Target {
        &self.values
    }
}
impl Writes {
    pub fn new(budget: SharedBudget) -> Self {
        Self {
            values: Vec::new(),
            charge: budget.account(AllocationClass::PaintMetadata),
        }
    }
    fn copy_checked(&self, limit: usize, check: &dyn Fn() -> Result<()>) -> Result<Self> {
        let mut copy = Self::new(self.charge.budget());
        let mut metadata = metadata::Metadata::new(0, limit)?;
        for chunk in self.values.chunks(1024) {
            check()?;
            for &write in chunk {
                metadata.vector(&mut copy.values, &mut copy.charge)?;
                copy.values.push(write);
            }
        }
        check()?;
        Ok(copy)
    }
}
impl PaintPlan {
    pub(super) fn unique_writes(&mut self, check: &dyn Fn() -> Result<()>) -> Result<&mut Writes> {
        if Arc::strong_count(&self.writes) != 1 {
            let copy = self.writes.copy_checked(self.metadata_limit, check)?;
            self.writes = Arc::new(copy);
        }
        Ok(Arc::get_mut(&mut self.writes).expect("private descriptor owner"))
    }
}
