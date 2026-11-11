//! Scoped descriptive installation discovery; never bake or resource authority.
use super::*;
use paintcraft_core::types::{json::Value, policy::hex};

pub const PROFILE: &str = "paintcraft.jxr-installation/v1";
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Identity {
    helper: [u8; 32],
    manifest: [u8; 32],
}
pub(super) fn identity(helper: &codec_helper::VerifiedEncoderHelper) -> Identity {
    Identity {
        helper: helper.sha256(),
        manifest: helper.manifest_sha256(),
    }
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Observation {
    Available(Identity),
    /// Missing or outside the exact host-owned writer contracts. Other errors
    /// (including cancellation/resource/storage) propagate without reclassification.
    Unavailable,
}
impl Observation {
    /// # Errors
    /// Returns CapabilityUnsupported for an unavailable observation.
    pub fn identity(self) -> Result<Identity> {
        match self {
            Self::Available(id) => Ok(id),
            Self::Unavailable => Err(Error::new(
                ErrorCode::CapabilityUnsupported,
                "JPEG XR installed writer unavailable or outside host contract",
            )),
        }
    }
    /// The exact same scoped projection is consumed by Native and CLI. It is
    /// descriptive; no boolean in this record permits rendering or publication.
    pub fn to_value(self) -> Value {
        let (available, helper, manifest) = match self {
            Self::Available(id) => (
                true,
                Value::str(hex(&id.helper)),
                Value::str(hex(&id.manifest)),
            ),
            Self::Unavailable => (false, Value::Null, Value::Null),
        };
        Value::object([
            ("schema", Value::str(PROFILE)),
            ("application_version", Value::str(env!("CARGO_PKG_VERSION"))),
            ("os", Value::str(std::env::consts::OS)),
            ("architecture", Value::str(std::env::consts::ARCH)),
            (
                "variant",
                Value::str(super::super::jxr_encode::qualified::EXPORT_VARIANT),
            ),
            ("available", Value::Bool(available)),
            (
                "reason",
                Value::str(if available {
                    "verified-host-contract"
                } else {
                    "missing-or-unqualified-installed-writer"
                }),
            ),
            ("helper_sha256", helper),
            ("manifest_sha256", manifest),
            ("release_qualified", Value::Bool(false)),
        ])
    }
}
/// Hashes the existing bounded installed helper/manifest through the same exact
/// host-owned V8/V10 contract set as JxrEncodeJob. Native calls only inside its
/// admitted background service (or existing final save preparation checkpoint).
/// # Errors
/// Preserves cancellation/resource/storage and changed-file errors verbatim.
pub fn observe(check: &dyn Fn() -> Result<()>) -> Result<Observation> {
    check()?;
    match super::writer_contracts::with_contracts(env!("CARGO_PKG_VERSION"), |contracts| {
        codec_helper::observe_encoder_set(contracts, check)
    }) {
        Ok(Some(helper)) => {
            helper.checked_executable(check)?;
            let id = identity(&helper);
            check()?;
            Ok(Observation::Available(id))
        }
        Ok(None) => {
            check()?;
            Ok(Observation::Unavailable)
        }
        Err(error) if error.code == ErrorCode::CapabilityUnsupported => {
            check()?;
            Ok(Observation::Unavailable)
        }
        Err(error) => Err(error),
    }
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Request(u64);
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Ticket {
    generation: u64,
    identity: Identity,
}
impl Ticket {
    pub fn observation(self) -> Observation {
        Observation::Available(self.identity)
    }
    pub fn generation(self) -> u64 {
        self.generation
    }
}
/// One host's scoped monotonic observation ledger; not a universal permission
/// service. Keep through dialog retirement so observed ABA invalidates old work.
/// Unknown external changes between checkpoints are not filesystem CAS.
#[derive(Debug, Default)]
pub struct Generation {
    requested: u64,
    applied: u64,
    epoch: u64,
    observation: Option<Observation>,
}
fn stale() -> Error {
    Error::new(
        ErrorCode::ConflictStaleBase,
        "JPEG XR installation generation changed",
    )
}
impl Generation {
    /// # Errors
    /// Refuses counter exhaustion instead of reusing an earlier request identity.
    pub fn begin_refresh(&mut self) -> Result<Request> {
        self.requested = self.requested.checked_add(1).ok_or_else(stale)?;
        Ok(Request(self.requested))
    }
    /// # Errors
    /// Rejects older/duplicate completions and epoch exhaustion. Unavailable
    /// observations are recorded before reporting capability refusal.
    pub fn complete(&mut self, request: Request, observation: Observation) -> Result<Ticket> {
        if request.0 != self.requested || request.0 <= self.applied {
            return Err(stale());
        }
        let epoch = if self.observation != Some(observation) {
            self.epoch.checked_add(1).ok_or_else(stale)?
        } else {
            self.epoch
        };
        self.observation = Some(observation);
        self.epoch = epoch;
        self.applied = request.0;
        Ok(Ticket {
            generation: epoch,
            identity: observation.identity()?,
        })
    }
    /// # Errors
    /// Refuses a pending refresh, old generation or changed verified identity.
    pub fn require_current(&self, ticket: Ticket) -> Result<()> {
        if self.requested != self.applied
            || self.epoch != ticket.generation
            || self.observation != Some(ticket.observation())
        {
            return Err(stale());
        }
        Ok(())
    }
}
#[cfg(test)]
mod tests;
