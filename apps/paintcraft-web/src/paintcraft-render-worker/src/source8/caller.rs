use super::*;
use paintcraft_render::{
    noise_work::{self, NoiseAdmission, NoiseJobPolicy, WorkComponents},
    spatial::{self, WorkspaceAdmission},
};
#[repr(C, u8)]
pub(super) enum RequiredCaller8 {
    Base(WorkspaceAdmission),
    Noise {
        selected: NoiseAdmission,
        ambient: NoiseAdmission,
    },
}
impl RequiredCaller8 {
    pub(super) fn capture(request: &Source8InspectionRequest) -> Result<Self> {
        NoiseJobPolicy::BrowserV1.validate(request.caller)?;
        let value = match request.mode {
            Source8InspectionMode::Base => {
                if request.caller.noise_work != 0
                    || noise_work::capture_current_noise_workspace().is_some()
                {
                    return Err(missing());
                }
                Self::Base(spatial::capture_required_workspace(
                    request.caller.bytes,
                    request.caller.base_work,
                )?)
            }
            Source8InspectionMode::Noise => {
                let ambient = noise_work::capture_current_noise_workspace().ok_or_else(missing)?;
                let selected = noise_work::capture_required_noise_workspace(request.caller)?;
                Self::Noise { selected, ambient }
            }
        };
        value.validate()?;
        Ok(value)
    }
    pub(super) fn validate(&self) -> Result<()> {
        match self {
            Self::Base(base) => base.reserve_work(0),
            Self::Noise { selected, .. } => selected.check_available(WorkComponents::default()),
        }
    }
    pub(super) fn spend(&self, n: u64) -> Result<()> {
        match self {
            Self::Base(base) => base.reserve_work(n),
            Self::Noise { selected, .. } => selected.reserve_base_work(n),
        }
    }
    pub(super) fn validate_byte_union(&self, bytes: u64) -> Result<()> {
        let (prepared, sidecar, limit) = match self {
            Self::Base(base) => (
                base.prepared_owner_union_bytes(None)?,
                0,
                base.bytes_limit(),
            ),
            Self::Noise { selected, ambient } => (
                selected
                    .workspace()
                    .prepared_owner_union_bytes(Some(ambient.workspace()))?,
                selected.retained_control_bytes(),
                selected.limits().bytes,
            ),
        };
        let total = bytes
            .checked_add(prepared)
            .and_then(|n| n.checked_add(sidecar))
            .ok_or_else(extent)?;
        if total > limit {
            return Err(extent());
        }
        self.validate()
    }
}
fn missing() -> Error {
    Error::new(
        ErrorCode::CapabilityUnsupported,
        "Source8 requires its matching caller",
    )
}
