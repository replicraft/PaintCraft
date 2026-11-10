//! One no-grow admitted encoded buffer. The public runtime only borrows its window.
use super::{job::OperationJob8, policy::*};
use paintcraft_core::types::{
    Result,
    resources::{BudgetAccount, BudgetScope},
};
#[repr(C)]
pub(super) struct Stage8 {
    pub data: Vec<u8>,
    _bytes: BudgetAccount,
    _controls: BudgetAccount,
    _encoded_factory: BudgetScope,
    _control_factory: BudgetScope,
}
impl Stage8 {
    pub fn new(
        n: u64,
        encoded: &BudgetScope,
        h: &BudgetScope,
        controls: u64,
        job: &OperationJob8,
        check: &dyn Fn() -> Result<()>,
    ) -> Result<Self> {
        // The production stage caller already prepaid complete containing slots and later-factory formation.
        job.spend(
            n.checked_mul(2)
                .and_then(|n| n.checked_add(512))
                .ok_or_else(bound)?,
        )?;
        check()?;
        let mut control = h.account();
        control.resize(controls)?;
        let mut bytes = encoded.account();
        bytes.resize(n)?;
        let n = usize::try_from(n).map_err(|_| bound())?;
        let mut data = Vec::new();
        data.try_reserve_exact(n).map_err(|_| bound())?;
        if data.capacity() != n {
            return Err(bound());
        }
        data.resize(n, 0);
        Ok(Self {
            data,
            _bytes: bytes,
            _controls: control,
            _encoded_factory: encoded.clone(),
            _control_factory: h.clone(),
        })
    }
}
