//! Terminal ownership transition. Existing entry bodies/layout stay unchanged.
use super::*;
use crate::source8::service_bridge::{self, OwnedReadFactories8, ServiceSource8};
impl Entry8<'_> {
    pub(in crate::source8) fn take_service_source_checked(&mut self) -> Result<ServiceSource8> {
        if !matches!(self.source, SourceSlot8::Loaded(_)) {
            return Err(incomplete());
        }
        self.auth.spend(64)?;
        service_bridge::validate_transfer()?;
        if self._control.bytes() != owner_ledger::ENTRY_BYTES {
            return Err(extent());
        }
        self.auth.step(service_bridge::transfer_work()?)?;
        let source_sha256 = self.inspect_checked()?.source_sha256;
        let factories = OwnedReadFactories8::capture_prepaid(&self.realms);
        // Account creates2 strong+1 weak increment;50 included both eventual
        // retirements and the22 cloned original-factory increments/decrements.
        let placeholder = self.realms.c.account();
        // No fallible operation, callback, allocator or new grant after the take.
        let source = std::mem::replace(&mut self.source, SourceSlot8::Failed);
        let SourceSlot8::Loaded(source) = source else {
            unreachable!("checked Loaded entry");
        };
        let control = std::mem::replace(&mut self._control, placeholder);
        Ok(ServiceSource8::from_prepaid_parts(
            source,
            self.realms.caps,
            source_sha256,
            control,
            factories,
        ))
    }
}
impl<'a> EntryAuth8<'a> {
    pub(in crate::source8) fn from_service_caller_checked(
        required: &caller::RequiredCaller8,
        raw_check: &'a dyn Fn() -> Result<()>,
    ) -> Result<Self> {
        required.spend(service_bridge::patch_auth_work()?)?;
        required.validate()?;
        let caller = service_bridge::clone_required_prepaid(required);
        Ok(Self { caller, raw_check })
    }
}

impl Entry8<'_> {
    pub(in crate::source8) fn service_match_checked(
        &self,
        expected: &crate::source8::service_endpoint::Expected8,
    ) -> Result<()> {
        self.auth.spend(8192)?;
        let named = 2 * size_of::<crate::source8::service_endpoint::Expected8>()
            + 2 * size_of::<Source8Inspection>()
            + 2 * size_of::<Result<Source8Inspection>>()
            + 4 * size_of::<Result<bool>>()
            + size_of::<[u64; 16]>()
            + 8 * size_of::<&u8>();
        if named as u64 > owner_ledger::ENTRY_BYTES {
            return Err(extent());
        }
        let SourceSlot8::Loaded(source) = &self.source else {
            return Err(incomplete());
        };
        let info = self.inspect_checked()?;
        let noise = source.service_noise_checked(&|n| self.auth.spend(n))?;
        if info.source_sha256() != expected.source_sha
            || info.document_id() != expected.document
            || info.state_id() != expected.state
            || info.canvas() != expected.canvas
            || noise != expected.stored_noise
        {
            return Err(Error::new(
                ErrorCode::ConflictStaleBase,
                "Source8 staged binding differs",
            ));
        }
        Ok(())
    }
}
