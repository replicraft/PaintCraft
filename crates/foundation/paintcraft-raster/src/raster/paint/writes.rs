//! Shared immutable descriptor storage; writes acquire independent charged copies.
use super::*;
use paintcraft_core::types::resources::{BudgetAccount, BudgetScope};

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
    pub fn new(scope: BudgetScope) -> Self {
        Self {
            values: Vec::new(),
            charge: scope.account(),
        }
    }
    fn copy_checked(&self, limit: usize, check: &dyn Fn() -> Result<()>) -> Result<Self> {
        let mut copy = Self {
            values: Vec::new(),
            charge: self.charge.sibling(),
        };
        let bytes = self
            .values
            .capacity()
            .checked_mul(std::mem::size_of::<Write>())
            .ok_or_else(metadata::exhausted)?;
        metadata::Metadata::new(bytes, limit)?;
        copy.charge.resize(bytes as u64)?;
        copy.values
            .try_reserve_exact(self.values.capacity())
            .map_err(|_| Error::new(ErrorCode::ResourceLimit, "cannot copy paint descriptors"))?;
        let actual = copy
            .values
            .capacity()
            .checked_mul(std::mem::size_of::<Write>())
            .ok_or_else(metadata::exhausted)?;
        copy.charge.resize(actual as u64)?;
        metadata::Metadata::new(actual, limit)?;
        for chunk in self.values.chunks(1024) {
            check()?;
            // Capacity was admitted above; this cannot allocate.
            copy.values.extend_from_slice(chunk);
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
