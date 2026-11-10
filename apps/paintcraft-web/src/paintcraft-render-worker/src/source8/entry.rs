use super::*;
use std::mem::size_of;
#[repr(C)]
pub(super) struct EntryAuth8<'r> {
    caller: caller::RequiredCaller8,
    raw_check: &'r dyn Fn() -> Result<()>,
}
impl EntryAuth8<'_> {
    pub(super) fn spend(&self, n: u64) -> Result<()> {
        self.caller.spend(n)
    }
    /// RAW check: qualified helpers already paid their own512 callback slot.
    pub(super) fn check(&self) -> Result<()> {
        self.caller.validate()?;
        (self.raw_check)()?;
        self.caller.validate()
    }
    pub(super) fn step(&self, n: u64) -> Result<()> {
        self.spend(n.checked_add(512).ok_or_else(extent)?)?;
        self.check()
    }
}
#[repr(C, u8)]
pub(super) enum SourceSlot8 {
    Empty,
    Reading,
    Loaded(owner::ClosedSource8),
    Failed,
}
#[repr(C)]
pub(super) struct Entry8<'r> {
    source: SourceSlot8,
    auth: EntryAuth8<'r>,
    request: Source8InspectionRequest,
    realms: Source8ReadRealms<'r>,
    _control: BudgetAccount,
}
impl<'r> Entry8<'r> {
    pub(super) fn new(
        request: Source8InspectionRequest,
        realms: Source8ReadRealms<'r>,
        raw_check: &'r dyn Fn() -> Result<()>,
    ) -> Result<Self> {
        let caller = caller::RequiredCaller8::capture(&request)?;
        let auth = EntryAuth8 { caller, raw_check };
        auth.spend(64)?;
        owner_ledger::validate_layouts()?;
        realms.validate_originals(&auth)?;
        auth.spend(64)?;
        auth.caller.validate_byte_union(realms.declared_union()?)?;
        if realms.caps.c < owner_ledger::ENTRY_BYTES {
            return Err(extent());
        }
        let mut control = realms.c.account();
        control.resize(owner_ledger::ENTRY_BYTES)?;
        auth.spend(2 * owner_ledger::ENTRY_BYTES)?;
        Ok(Self {
            source: SourceSlot8::Empty,
            auth,
            request,
            realms,
            _control: control,
        })
    }
    pub(super) fn read_into_slot_checked(&mut self, bytes: &[u8]) -> Result<()> {
        if !matches!(self.source, SourceSlot8::Empty) {
            return Err(incomplete());
        }
        self.source = SourceSlot8::Reading;
        let result = build::read_payload_checked(bytes, &self.request, &self.realms, &self.auth)
            .and_then(|payload| owner::close_checked(payload, &self.auth, &self.realms));
        match result {
            Ok(source) => {
                self.source = SourceSlot8::Loaded(source);
                Ok(())
            }
            Err(error) => {
                self.source = SourceSlot8::Failed;
                Err(error)
            }
        }
    }
    pub(super) fn inspect_checked(&self) -> Result<Source8Inspection> {
        let SourceSlot8::Loaded(source) = &self.source else {
            return Err(incomplete());
        };
        self.auth.step(2 * size_of::<Source8Inspection>() as u64)?;
        source.inspect_scalar()
    }
    #[cfg(test)]
    pub(super) fn clone_for_retirement_checked(&self) -> Result<owner::Retirement8<'_>> {
        let SourceSlot8::Loaded(source) = &self.source else {
            return Err(incomplete());
        };
        self.auth.spend(64)?;
        let debit = 512 + 2 * size_of::<owner::ClosedSource8>() as u64 + 2 + 64;
        self.auth.spend(debit)?;
        self.auth.check()?;
        Ok(source.clone_for_retirement())
    }
}

#[path = "frame_entry.rs"]
mod frame_entry;

#[path = "display_entry.rs"]
mod display_entry;

#[cfg(test)]
#[path = "tests/build_cases.rs"]
mod build_cases;
#[cfg(test)]
#[path = "tests/entry_cases.rs"]
mod entry_cases;
