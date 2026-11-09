//! Separate original frame roles. Scalars group/check capacity, never grant it.
use super::*;
use std::{fmt, mem::size_of};
pub(super) const CONTROL_BYTES: u64 = 16384;
#[repr(C)]
#[derive(Clone, Copy, Debug)]
pub struct Source8FrameCaps {
    pub workspace: u64,
    pub control: u64,
    pub output: u64,
}
#[repr(C)]
pub struct Source8FrameScopes<'a> {
    pub control: &'a BudgetScope,
    pub output: &'a BudgetScope,
}
impl fmt::Debug for Source8FrameScopes<'_> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Source8FrameScopes")
            .field("factories", &2)
            .finish()
    }
}
#[repr(C)]
pub struct Source8FrameRealms<'a> {
    pub(super) control: &'a BudgetScope,
    pub(super) output: &'a BudgetScope,
    pub(super) caps: Source8FrameCaps,
}
impl<'a> Source8FrameRealms<'a> {
    pub fn new(scopes: Source8FrameScopes<'a>, caps: Source8FrameCaps) -> Self {
        Self {
            control: scopes.control,
            output: scopes.output,
            caps,
        }
    }
    /// Initial F snapshot is covered by the separate reader-entry bootstrap
    /// shadow. No W/O/full-frame state is formed until the F account exists.
    pub(super) fn admit_control<'f>(
        &'f self,
        auth: &entry::EntryAuth8<'_>,
    ) -> Result<FrameControl8<'f, 'a>> {
        auth.spend(112)?;
        {
            let snapshot = self.control.capacity_snapshot();
            if !snapshot.reserved
                || snapshot.ceiling != self.caps.control
                || self.caps.control < CONTROL_BYTES
            {
                return Err(invalid());
            }
        }
        auth.spend(6 + 2 * size_of::<FrameControl8<'static, 'static>>() as u64)?;
        let mut account = self.control.account();
        account.resize(CONTROL_BYTES)?;
        Ok(FrameControl8 {
            account,
            realms: self,
        })
    }
}
impl fmt::Debug for Source8FrameRealms<'_> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Source8FrameRealms")
            .field("caps", &self.caps)
            .finish_non_exhaustive()
    }
}
#[repr(C)]
pub(super) struct FrameControl8<'f, 'r> {
    account: BudgetAccount,
    realms: &'f Source8FrameRealms<'r>,
}
/// Supplied ONLY by the captured private caller/read owner, not a public recipe.
#[repr(C)]
pub(super) struct FrameValidation8<'a> {
    pub workspace: &'a BudgetScope,
    pub read_capacity_sum: u64,
    pub caller_bytes: u64,
    pub prepared_union: u64,
}
impl FrameControl8<'_, '_> {
    pub(super) fn validate_originals(
        &self,
        ctx: FrameValidation8<'_>,
        auth: &entry::EntryAuth8<'_>,
    ) -> Result<()> {
        if self.account.bytes() != CONTROL_BYTES {
            return Err(invalid());
        }
        for (scope, cap) in [
            (ctx.workspace, self.realms.caps.workspace),
            (self.realms.output, self.realms.caps.output),
        ] {
            auth.spend(112)?;
            let snapshot = scope.capacity_snapshot();
            if !snapshot.reserved || snapshot.ceiling != cap {
                return Err(invalid());
            }
        }
        auth.spend(64)?;
        let total = [
            ctx.read_capacity_sum,
            self.realms.caps.workspace,
            self.realms.caps.control,
            self.realms.caps.output,
            ctx.prepared_union,
        ]
        .into_iter()
        .try_fold(0u64, |a, b| a.checked_add(b).ok_or_else(invalid))?;
        if total > ctx.caller_bytes {
            return Err(invalid());
        }
        Ok(())
    }
}
pub(super) fn invalid() -> Error {
    Error::new(
        ErrorCode::ResourceLimit,
        "Source8 frame original declaration limit",
    )
}

/// Demand-only precheck, from the named Loaded-source C phase before F resize.
/// Actual immutable W/O snapshots/equality still run after F has been funded.
pub(super) fn validate_declared(ctx: &FrameValidation8<'_>, caps: &Source8FrameCaps) -> Result<()> {
    let total = [
        ctx.read_capacity_sum,
        caps.workspace,
        caps.control,
        caps.output,
        ctx.prepared_union,
    ]
    .into_iter()
    .try_fold(0u64, |a, b| a.checked_add(b).ok_or_else(invalid))?;
    if total > ctx.caller_bytes {
        return Err(invalid());
    }
    Ok(())
}
