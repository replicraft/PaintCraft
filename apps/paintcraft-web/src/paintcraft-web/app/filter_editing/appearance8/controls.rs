//! Owning Main controls: actual target types, not Worker fixture constants.
use super::{
    capture::{CurrentStamp8, Expected8, Identity8},
    denied,
    factory::{Factory8, FactoryData8},
    frame::{FrameStage8, Funding8, FundingData8},
    operation::{Operation8, Parent8},
    publication::{CpuPublication8, Retirement8},
    snapshot::Snapshot8,
};
use eframe::egui::ColorImage;
use paintcraft_core::types::{
    Result,
    resources::{BudgetAccount, BudgetReservation, BudgetScope, ScopeCapacity},
};
use std::mem::{align_of, size_of};
fn sum(values: &[u64]) -> Result<u64> {
    values
        .iter()
        .try_fold(0u64, |a, n| a.checked_add(*n).ok_or_else(denied))
}
fn bytes<T>() -> u64 {
    size_of::<T>() as u64
}
// Selected logical std Rc/Arc inline convention; no Weak escapes for these
// private owners. Allocator headers/slack and lazy runtime backing excluded.
pub(super) fn shared<T>() -> Result<u64> {
    let alignment = align_of::<T>().max(align_of::<usize>());
    let start = (2 * size_of::<usize>())
        .checked_next_multiple_of(align_of::<T>())
        .ok_or_else(denied)?;
    let size = start
        .checked_add(size_of::<T>())
        .and_then(|n| n.checked_next_multiple_of(alignment))
        .ok_or_else(denied)?;
    u64::try_from(size).map_err(|_| denied())
}
pub(super) fn frame_bytes() -> Result<u64> {
    sum(&[
        2 * bytes::<FrameStage8>(),
        2 * bytes::<Result<FrameStage8>>(),
        2 * bytes::<Funding8>(),
        2 * bytes::<Result<Funding8>>(),
        shared::<FundingData8>()?,
        2 * bytes::<CpuPublication8>(),
        2 * bytes::<Result<CpuPublication8>>(),
        2 * bytes::<Retirement8>(),
        2 * bytes::<Result<Retirement8>>(),
        2 * bytes::<ColorImage>(),
        shared::<ColorImage>()?,
        2 * bytes::<Operation8>(),
        2 * bytes::<Result<Operation8>>(),
        2 * bytes::<BudgetAccount>(),
        4 * bytes::<ScopeCapacity>(),
        bytes::<[Result<()>; 16]>(),
        bytes::<[u64; 32]>(),
        bytes::<[&u8; 16]>(),
    ])
}
pub(super) fn factory_bytes() -> Result<u64> {
    #[cfg(target_arch = "wasm32")]
    let opaque = sum(&[
        super::owner::control_bytes()?,
        super::bridge::control_bytes()?,
        super::display::control_bytes()?,
    ])?;
    #[cfg(not(target_arch = "wasm32"))]
    let opaque = 0;
    sum(&[
        opaque,
        shared::<FactoryData8>()?,
        4 * bytes::<Factory8>(),
        2 * bytes::<Result<Factory8>>(),
        2 * bytes::<Parent8>(),
        2 * bytes::<Operation8>(),
        2 * bytes::<Result<Operation8>>(),
        BudgetReservation::control_allocation_quote()?,
        BudgetScope::control_allocation_quote()?,
        paintcraft_render::spatial::WorkspaceAdmission::control_allocation_quote()?,
        // Fixed bounded Rust source/ready/open metadata carrier shadow, separate
        // from the controller's defining JS57856. No JS object heap claim.
        bytes::<[[u8; 4096]; 2]>(),
        bytes::<[[u8; 2048]; 2]>(),
        bytes::<[Result<()>; 16]>(),
        bytes::<[u64; 32]>(),
        bytes::<[&u8; 16]>(),
    ])
}
pub(super) fn snapshot_bytes() -> Result<u64> {
    sum(&[
        2 * bytes::<Snapshot8>(),
        2 * bytes::<Result<Snapshot8>>(),
        4 * bytes::<Identity8>(),
        4 * bytes::<CurrentStamp8>(),
        bytes::<[Result<()>; 16]>(),
        bytes::<[u64; 32]>(),
        bytes::<[&u8; 16]>(),
    ])
}
pub(super) fn match_work() -> u64 {
    16384
}
fn match_layout() -> Result<()> {
    let named = 6 * bytes::<Option<Expected8>>()
        + 4 * bytes::<Expected8>()
        + 4 * bytes::<Identity8>()
        + 4 * bytes::<Result<Identity8>>()
        + bytes::<[String; 24]>()
        + bytes::<[Option<String>; 16]>()
        + 64 * 112
        + 8 * 112
        + bytes::<[u64; 24]>()
        + 552
        + 1024;
    if named > match_work() {
        return Err(denied());
    }
    Ok(())
}
pub(super) fn validate() -> Result<()> {
    match_layout()?;
    if !matches!(size_of::<usize>(), 4 | 8)
        || size_of::<paintcraft_core::types::Error>() > 96
        || factory_bytes()? > 32768
        || frame_bytes()? > 32768
        || snapshot_bytes()? > 32768
    {
        return Err(denied());
    }
    Ok(())
}
