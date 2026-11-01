//! Explicit root selection from the same frozen source and parsed batch executed below.
use super::*;
use paintcraft_commands::{Batch, FilterOp, Op};
use paintcraft_core::types::filters::Descriptor;
use paintcraft_render::noise_work::{
    NoiseAdmission, NoiseJobPolicy, NoiseLimits, capture_noise_or_current, with_noise_workspace,
};
const NOISE_WORK: u64 = 1 << 35;
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Mode {
    Legacy,
    Noise,
}
impl Mode {
    pub(crate) fn capture(doc: &Document, batch: Option<&Batch>) -> Self {
        let stored = doc.layers.values().any(|layer| {
            layer
                .filters
                .iter()
                .any(|filter| matches!(filter.descriptor(), Descriptor::AddNoiseRgbV1(_)))
        });
        let prospective = batch.is_some_and(|batch| {
            batch.operations.iter().any(|op| {
                matches!(
                    op,
                    Op::Filter(
                        FilterOp::Add {
                            descriptor: Descriptor::AddNoiseRgbV1(_),
                            ..
                        } | FilterOp::Update {
                            descriptor: Descriptor::AddNoiseRgbV1(_),
                            ..
                        }
                    )
                )
            })
        });
        if stored || prospective {
            Self::Noise
        } else {
            Self::Legacy
        }
    }
}
pub(crate) fn run_captured<T>(mode: Mode, work: impl FnOnce() -> Result<T>) -> Result<T> {
    in_budget(
        mode,
        realm(),
        NoiseLimits {
            bytes: BYTES,
            base_work: WORK,
            noise_work: NOISE_WORK,
        },
        work,
    )
}
fn in_budget<T>(
    mode: Mode,
    budget: &SharedBudget,
    limits: NoiseLimits,
    work: impl FnOnce() -> Result<T>,
) -> Result<T> {
    if mode == Mode::Legacy {
        let admission = paintcraft_render::spatial::capture_workspace_or_current(
            || {
                WorkspaceAdmission::new(
                    budget.scope(AllocationClass::SharedBudget),
                    limits.bytes,
                    limits.base_work,
                )
            },
            limits.bytes,
            limits.base_work,
        )?;
        return with_workspace(admission, work);
    }
    let admission = capture_noise_or_current(
        || {
            NoiseAdmission::new_root(
                budget.scope(AllocationClass::SharedBudget),
                NoiseJobPolicy::NativeV1,
                limits,
            )
        },
        limits,
    )?;
    with_noise_workspace(admission, work)
}
#[cfg(test)]
mod tests;
