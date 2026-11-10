//! Original C-backed serial accumulator; storage retires before credit/factory.
use super::*;
use paintcraft_render::display::DisplayAccumulator;
#[repr(C)]
pub(super) struct Scratch {
    pub values: Vec<DisplayAccumulator>,
    _account: BudgetAccount,
    _factory: BudgetScope,
}
impl Scratch {
    pub fn new(
        cells: usize,
        bytes: u64,
        scope: &BudgetScope,
        auth: &entry::EntryAuth8<'_>,
    ) -> Result<Self> {
        auth.spend(1024)?;
        let factory = scope.clone();
        let mut account = factory.account();
        account.resize(bytes)?;
        let mut values = Vec::new();
        values.try_reserve_exact(cells).map_err(|_| invalid())?;
        if values.capacity() != cells {
            return Err(invalid());
        }
        values.resize(cells, DisplayAccumulator::default());
        Ok(Self {
            values,
            _account: account,
            _factory: factory,
        })
    }
}
