//! Optional, admitted len-one native state. Default/browser Source allocates none.
use super::*;
use paintcraft_input::tablet::receiver::CaptureStatus;
#[derive(Debug, Default)]
pub(crate) struct Resident {
    value: Vec<Metadata>,
    // Allocation and its contents drop before their logical admission.
    _storage: Option<BudgetAccount>,
}
impl Resident {
    pub(crate) fn new(budget: &SharedBudget, capacity: usize) -> Result<Self> {
        let mut storage = budget.account(AllocationClass::PaintMetadata);
        storage.resize(state::storage_bytes() as u64)?;
        let mut value = Vec::new();
        value
            .try_reserve_exact(1)
            .map_err(|_| invalid(Invalid::Capacity))?;
        storage.resize((value.capacity() * std::mem::size_of::<Metadata>()) as u64)?;
        let frames = budget
            .reserve_capacity((capacity * std::mem::size_of::<NativeTouchBatch>()) as u64)?
            .scope(AllocationClass::PaintMetadata);
        let bank = budget
            .reserve_capacity(state::BANK_BYTES as u64)?
            .scope(AllocationClass::PaintMetadata);
        value.push(Metadata::new(frames, bank)?);
        Ok(Self {
            value,
            _storage: Some(storage),
        })
    }
    pub(crate) fn active(&self) -> bool {
        self.value.first().is_some_and(Metadata::active)
    }
    pub(crate) fn fence_all(&mut self) {
        for metadata in &mut self.value {
            metadata.fence_all();
        }
    }
    pub(crate) fn fence_canvas(&mut self) {
        for metadata in &mut self.value {
            metadata.fence_canvas();
        }
    }
    pub(crate) fn deny(&mut self, id: Identity) {
        for metadata in &mut self.value {
            metadata.deny(id);
        }
    }
    pub(super) fn observe(
        &mut self,
        m: NativeFingerMetadata,
        admission: NativeFingerAdmission,
        context: Option<CaptureContext>,
        capture: CaptureStatus,
        convert: impl FnOnce(f64) -> std::result::Result<Option<(f64, f64)>, Invalid>,
    ) -> std::result::Result<Change, Invalid> {
        self.value
            .first_mut()
            .ok_or(Invalid::Owner)?
            .observe(m, admission, context, capture, convert)
    }
    pub(crate) fn frame_account(&self) -> std::result::Result<BudgetAccount, Invalid> {
        Ok(self.value.first().ok_or(Invalid::Owner)?.frames.account())
    }
    pub(crate) fn retire_all(
        &mut self,
    ) -> std::result::Result<[Option<NativeTouchCancellation>; 32], Invalid> {
        Ok(self.value.first_mut().ok_or(Invalid::Owner)?.retire_all())
    }
    pub(crate) fn retire_identity(
        &mut self,
        id: Identity,
        installation: u64,
        ordinal: u64,
        time: Time,
    ) -> std::result::Result<(), Invalid> {
        self.value
            .first_mut()
            .ok_or(Invalid::Owner)?
            .retire_identity(id, installation, ordinal, time)
    }
    pub(crate) fn validate_continuation(&self, lease: &Lease, proof: Predecessor) -> bool {
        self.value
            .first()
            .is_some_and(|m| m.validate_continuation(lease, proof))
    }
    pub(crate) fn acknowledge(&mut self, lease: &Lease, after: Owner) {
        self.value
            .first_mut()
            .expect("validated native touch source")
            .acknowledge(lease, after);
    }
}
