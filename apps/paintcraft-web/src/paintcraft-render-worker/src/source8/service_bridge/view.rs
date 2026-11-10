//! Same captured required owner across patches; callback exists only on stack.
use super::*;
impl ServiceSource8 {
    /// The private operation already captured ONE actual required caller. This
    /// does not recapture or install TLS and cannot mint a dispatch authority.
    pub(in crate::source8) fn begin_view_checked(
        &self,
        required: &caller::RequiredCaller8,
        originals: &ServiceOriginals8<'_>,
        frame: &Source8FrameRealms<'_>,
        check: &dyn Fn() -> Result<()>,
    ) -> Result<ServiceView8> {
        required.spend(64)?;
        layout::validate()?;
        if self._control.bytes() != owner_ledger::ENTRY_BYTES {
            return Err(extent());
        }
        required.spend(2 * layout::begin_named()? + 50)?;
        for (scope, cap) in [
            (originals.encoded, originals.encoded_capacity),
            (originals.control, originals.control_capacity),
        ] {
            required.spend(112)?;
            let actual = scope.capacity_snapshot();
            if !actual.reserved || actual.ceiling != cap {
                return Err(extent());
            }
        }
        let workspace = match required {
            caller::RequiredCaller8::Base(base) => base.scope(),
            caller::RequiredCaller8::Noise { selected, .. } => selected.workspace().scope(),
        };
        for (scope, cap) in [
            (workspace, frame.caps.workspace),
            (frame.control, frame.caps.control),
            (frame.output, frame.caps.output),
        ] {
            required.spend(112)?;
            let actual = scope.capacity_snapshot();
            if !actual.reserved || actual.ceiling != cap {
                return Err(extent());
            }
        }
        required.spend(64)?;
        let total = self
            .read_bytes()?
            .checked_add(originals.encoded_capacity)
            .and_then(|n| n.checked_add(originals.control_capacity))
            .and_then(|n| n.checked_add(frame.caps.workspace))
            .and_then(|n| n.checked_add(frame.caps.control))
            .and_then(|n| n.checked_add(frame.caps.output))
            .ok_or_else(extent)?;
        required.validate_byte_union(total)?;
        if originals.control_capacity < layout::VIEW_CONTROL {
            return Err(extent());
        }
        required.spend(512)?;
        required.validate()?;
        check()?;
        required.validate()?;
        let mut control = originals.control.account();
        control.resize(layout::VIEW_CONTROL)?;
        // Account and full containing/move policy paid BEFORE these clones.
        Ok(ServiceView8 {
            caller: clone_required_prepaid(required),
            source_sha256: self.source_sha256,
            _encoded: originals.encoded.clone(),
            encoded_capacity: originals.encoded_capacity,
            control_capacity: originals.control_capacity,
            failed: false,
            control,
            _control_factory: originals.control.clone(),
        })
    }
}
impl ServiceView8 {
    pub(in crate::source8) fn render_patch_checked(
        &mut self,
        source: &ServiceSource8,
        frame: &Source8FrameRealms<'_>,
        display: &Source8DisplayPatch,
        check: &dyn Fn() -> Result<()>,
    ) -> Result<Source8RgbaImage> {
        if self.failed {
            return Err(incomplete());
        }
        // Bounded terminal failure bookkeeping, like Entry's Failed transition;
        // it refunds no work/capacity and never installs or renews a counter.
        self.failed = true;
        let result = self.patch_inner(source, frame, display, check);
        if result.is_ok() {
            self.failed = false;
        }
        result
    }
    fn patch_inner(
        &self,
        source: &ServiceSource8,
        frame: &Source8FrameRealms<'_>,
        display: &Source8DisplayPatch,
        check: &dyn Fn() -> Result<()>,
    ) -> Result<Source8RgbaImage> {
        self.caller.spend(64)?;
        if self.control.bytes() != layout::VIEW_CONTROL {
            return Err(extent());
        }
        let auth = entry::EntryAuth8::from_service_caller_checked(&self.caller, check)?;
        auth.step(32)?;
        if self.source_sha256 != source.source_sha256 {
            return Err(Error::new(
                ErrorCode::ConflictStaleBase,
                "Source8 service source changed",
            ));
        }
        auth.spend(64)?;
        let sidecar = match &self.caller {
            caller::RequiredCaller8::Base(_) => 0,
            caller::RequiredCaller8::Noise { selected, .. } => selected.retained_control_bytes(),
        };
        let read_bytes = source
            .read_bytes()?
            .checked_add(self.encoded_capacity)
            .and_then(|n| n.checked_add(self.control_capacity))
            .and_then(|n| n.checked_add(sidecar))
            .ok_or_else(extent)?;
        auth.spend(display_public::ORIGINAL_CONTROL_MOVE_WORK)?;
        let original_c = display_public::OriginalControl8 {
            scope: &source.factories.c,
            capacity: source.caps.c,
        };
        source.source.render_display_closed_checked(
            &auth,
            &self.caller,
            frame,
            original_c,
            read_bytes,
            *display,
        )
    }
}
