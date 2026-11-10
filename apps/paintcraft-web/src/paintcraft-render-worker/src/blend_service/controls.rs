//! Defining fixed logical service/phase declarations, never usage-derived grants.
use super::*;
use crate::source8::{
    self,
    service_endpoint::{Expected8, LoadedSource8, LoadedView8, Originals8},
};
use manifest::{Binding8, Command8, Open8};
use operation::{ActiveView8, Grid8, PreparedView8};
use paintcraft_core::types::resources::{BudgetReservation, ScopeCapacity, ScopeFootprint};
use paintcraft_render::spatial::WorkspaceAdmission;
use std::mem::size_of;
#[repr(C)]
#[derive(Clone, Copy, Debug)]
pub(super) struct CompiledControls8 {
    pub service_bytes: u64,
    pub stage_bytes: u64,
    pub view_bytes: u64,
}
fn sum(values: &[u64]) -> Result<u64> {
    values
        .iter()
        .try_fold(0u64, |a, n| a.checked_add(*n).ok_or_else(bound))
}
fn size<T>() -> u64 {
    size_of::<T>() as u64
}
fn shared_locals() -> Result<u64> {
    sum(&[
        2 * size::<Command8>(),
        2 * size::<Result<Command8>>(),
        2 * size::<Open8>(),
        2 * size::<Binding8>(),
        2 * size::<BudgetAccount>(),
        4 * size::<ScopeCapacity>(),
        2 * size::<ScopeFootprint>(),
        size::<[u64; 32]>(),
        size::<[usize; 16]>(),
        8 * size::<Result<u64>>(),
        8 * size::<Result<()>>(),
        16 * size::<&u8>(),
        8 * size::<&[u8]>(),
    ])
}
pub(super) fn future_factory_allocations() -> Result<u64> {
    // H and W original factories were the fixed bootstrap. All others follow payment.
    sum(&[
        10 * BudgetReservation::control_allocation_quote()?,
        8 * BudgetScope::control_allocation_quote()?,
    ])
}
impl CompiledControls8 {
    pub fn query() -> Result<Self> {
        if !matches!(size_of::<usize>(), 4 | 8)
            || size::<Error>() > 96
            || !paintcraft_render::rooted_frame_inline_layout_supported()
        {
            return Err(bound());
        }
        let factory = sum(&[
            SharedBudget::control_allocation_quote()?,
            12 * BudgetReservation::control_allocation_quote()?,
            10 * BudgetScope::control_allocation_quote()?,
        ])?;
        let service = sum(&[
            size::<LayerBlendService>(),
            size::<Result<LayerBlendService>>(),
            4 * size::<Self>(),
            2 * size::<Result<Self>>(),
            2 * size::<Sha256>(),
            2 * size::<[u8; 32]>(),
            2 * size::<[u8; WINDOW]>(),
            4 * size::<BudgetScope>(),
            4 * size::<SharedBudget>(),
            2 * size::<Result<BudgetReservation>>(),
            2 * size::<Result<BudgetScope>>(),
            shared_locals()?,
            factory,
        ])?;
        let counter = WorkspaceAdmission::control_allocation_quote()?;
        let stage = sum(&[
            2 * size::<Stage8>(),
            2 * size::<Result<Stage8>>(),
            2 * size::<Roles8>(),
            2 * size::<Result<Roles8>>(),
            2 * size::<LoadedSource8>(),
            2 * size::<Result<LoadedSource8>>(),
            2 * size::<source8::Source8InspectionRequest>(),
            2 * size::<source8::Source8ReadRealms<'static>>(),
            2 * size::<Expected8>(),
            2 * size::<source8::Source8Inspection>(),
            2 * size::<Result<source8::Source8Inspection>>(),
            2 * size::<Sha256>(),
            2 * size::<[u8; 32]>(),
            2 * size::<OperationJob8>(),
            2 * size::<Result<OperationJob8>>(),
            2 * size::<Vec<u8>>(),
            shared_locals()?,
            counter,
        ])?;
        let view = sum(&[
            2 * size::<ActiveView8>(),
            2 * size::<Result<ActiveView8>>(),
            2 * size::<PreparedView8>(),
            2 * size::<Result<PreparedView8>>(),
            2 * size::<LoadedView8>(),
            2 * size::<Result<LoadedView8>>(),
            2 * size::<OperationJob8>(),
            2 * size::<Result<OperationJob8>>(),
            2 * size::<Grid8>(),
            2 * size::<Result<Grid8>>(),
            2 * size::<source8::Source8FrameRealms<'static>>(),
            2 * size::<Originals8<'static>>(),
            2 * size::<source8::Source8DisplayPatch>(),
            2 * size::<Result<source8::Source8DisplayPatch>>(),
            2 * size::<Source8RgbaImage>(),
            2 * size::<Result<Source8RgbaImage>>(),
            shared_locals()?,
            4096,
            counter,
        ])?;
        let out = Self {
            service_bytes: service,
            stage_bytes: stage,
            view_bytes: view,
        };
        if out.total()? > JOB {
            return Err(bound());
        }
        Ok(out)
    }
    pub fn total(self) -> Result<u64> {
        sum(&[self.service_bytes, self.stage_bytes, self.view_bytes])
    }
    pub fn stage_named(self) -> Result<u64> {
        self.stage_bytes
            .checked_sub(WorkspaceAdmission::control_allocation_quote()?)
            .ok_or_else(bound)
    }
    pub fn view_named(self) -> Result<u64> {
        self.view_bytes
            .checked_sub(WorkspaceAdmission::control_allocation_quote()?)
            .ok_or_else(bound)
    }
    pub fn stage_formation_work(self) -> Result<u64> {
        sum(&[
            self.stage_named()?.checked_mul(2).ok_or_else(bound)?,
            future_factory_allocations()?
                .checked_mul(2)
                .ok_or_else(bound)?,
            FIXED,
            RETIRE,
        ])
    }
    pub fn patch_work(self) -> Result<u64> {
        let slots = sum(&[
            shared_locals()?,
            2 * size::<source8::Source8DisplayPatch>(),
            2 * size::<Result<source8::Source8DisplayPatch>>(),
            2 * size::<Source8RgbaImage>(),
            2 * size::<Result<Source8RgbaImage>>(),
            2 * size::<source8::Source8FrameRealms<'static>>(),
        ])?;
        slots
            .checked_mul(2)
            .and_then(|n| n.checked_add(FIXED))
            .ok_or_else(bound)
    }
    pub fn profile(self) -> [u8; 32] {
        let mut hash = Sha256::new();
        hash.update(b"paintcraft.worker-source8-service/v1\0");
        for n in VERSIONS {
            hash.update(n.to_le_bytes());
        }
        for n in LIMITS {
            hash.update(n.to_le_bytes());
        }
        for n in [self.service_bytes, self.stage_bytes, self.view_bytes] {
            hash.update(n.to_le_bytes());
        }
        hash.finalize().into()
    }
}
pub(super) const VERSIONS: [u32; 11] = [8, 4, 8, 2, 1, 1, 1, 1, 1, 64, 1];
pub(super) const LIMITS: [u64; 8] = [JOB, JOB, BASE, NOISE, PAYLOAD, 256 << 20, CELLS, 64];
