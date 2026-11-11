//! Explicit root choice retained with an immutable preview capture.
use super::*;
use paintcraft_core::types::{Error, ErrorCode, filters::Descriptor};
use paintcraft_render::noise_work::{
    NoiseAdmission, NoiseJobPolicy, NoiseLimits, capture_noise_or_current, with_noise_workspace,
};
const NOISE_WORK: u64 = 1 << 35;
// Captured in existing task closures, not added to a priced document/renderer type.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum JobPolicy {
    None,
    Legacy,
    Noise,
}
fn stored(doc: &Document) -> bool {
    doc.layers.values().any(|layer| {
        layer
            .filters
            .iter()
            .any(|filter| matches!(filter.descriptor(), Descriptor::AddNoiseRgbV1(_)))
    })
}
impl JobPolicy {
    pub(crate) fn capture(
        doc: &Document,
        prospective: Option<&Descriptor>,
        required: bool,
    ) -> Self {
        if stored(doc) || prospective.is_some_and(|d| matches!(d, Descriptor::AddNoiseRgbV1(_))) {
            Self::Noise
        } else if required || doc.has_styles() {
            Self::Legacy
        } else {
            Self::None
        }
    }
    pub(crate) fn capture_with_style(
        doc: &Document,
        prospective: Option<&Descriptor>,
        required: bool,
        prospective_style: bool,
    ) -> Self {
        Self::capture(doc, prospective, required || prospective_style)
    }
    pub(crate) fn run<T>(self, work: impl FnOnce() -> Result<T>) -> Result<T> {
        match self {
            Self::None => work(),
            Self::Legacy => {
                let admission = paintcraft_render::spatial::capture_workspace_or_current(
                    || {
                        WorkspaceAdmission::new(
                            budget().scope(AllocationClass::SharedBudget),
                            BYTES,
                            WORK,
                        )
                    },
                    BYTES,
                    WORK,
                )?;
                with_workspace(admission, work)
            }
            Self::Noise => {
                let limits = NoiseLimits {
                    bytes: BYTES,
                    base_work: WORK,
                    noise_work: NOISE_WORK,
                };
                let admission = capture_noise_or_current(
                    || {
                        NoiseAdmission::new_root(
                            budget().scope(AllocationClass::SharedBudget),
                            NoiseJobPolicy::NativeV1,
                            limits,
                        )
                    },
                    limits,
                )?;
                with_noise_workspace(admission, work)
            }
        }
    }
}
/// These routes have no admitted asynchronous Noise preparation yet.
pub(crate) fn refuse_noise(doc: &Document) -> Result<()> {
    if stored(doc) {
        return Err(Error::new(
            ErrorCode::CapabilityUnsupported,
            "Noise requires an admitted asynchronous capture for this native route",
        ));
    }
    Ok(())
}
pub(crate) fn legacy_document<T>(doc: &Document, work: impl FnOnce() -> Result<T>) -> Result<T> {
    refuse_noise(doc)?;
    JobPolicy::capture(doc, None, needed(doc)).run(work)
}
#[cfg(test)]
#[path = "noise/tests.rs"]
mod tests;
