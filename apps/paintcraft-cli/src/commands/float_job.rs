//! Shared isolated float preparation; no alternative decoder or publication engine.
use super::float_options::Selection;
use super::*;
use paintcraft_core::types::resources::{AllocationClass, BudgetAccount, ResourceLease};
use paintcraft_formats::{
    codec_worker as wire, float_import, precision,
    registry::{self, Family, Frontend, Operation, Platform, Request},
};
use paintcraft_platform::source::SourceIdentity;
use paintcraft_runtime::{Executor, Limits, SharedBudget};
use std::{sync::Arc, time::Duration};
pub(super) type Check = Arc<dyn Fn() -> Result<()> + Send + Sync>;
pub(super) const METADATA_BYTES: u64 = 8 << 20;
pub(super) fn digest(value: &str) -> Result<[u8; 32]> {
    if value.len() != 64
        || !value
            .bytes()
            .all(|v| v.is_ascii_digit() || (b'a'..=b'f').contains(&v))
    {
        return Err(Error::new(
            ErrorCode::InputSchema,
            "Expected digest must be 64 lowercase hexadecimal characters",
        ));
    }
    let mut bytes = [0; 32];
    for (i, byte) in bytes.iter_mut().enumerate() {
        *byte = u8::from_str_radix(&value[i * 2..i * 2 + 2], 16)
            .map_err(|_| Error::new(ErrorCode::InputSchema, "Invalid expected digest"))?;
    }
    Ok(bytes)
}
pub(super) fn profile(path: &Path) -> Result<(Family, wire::Codec, &'static str)> {
    match path
        .extension()
        .and_then(|v| v.to_str())
        .and_then(Family::from_extension)
    {
        Some(Family::Hdr) => Ok((
            Family::Hdr,
            wire::Codec::HdrSdr8,
            wire::NATIVE_HDR_SDR_IMPORT_VARIANT,
        )),
        Some(Family::Exr) => Ok((
            Family::Exr,
            wire::Codec::ExrSdr8,
            wire::NATIVE_EXR_SDR_IMPORT_VARIANT,
        )),
        _ => Err(Error::new(
            ErrorCode::CapabilityUnsupported,
            "Float conversion requires an HDR or EXR source",
        )),
    }
}
pub(super) fn admit(path: &Path, frontend: Frontend, codec: wire::Codec) -> Result<()> {
    let (family, selected, variant) = profile(path)?;
    if selected != codec {
        return Err(Error::new(
            ErrorCode::InputSchema,
            "Float source family changed",
        ));
    }
    let mut request = Request::new(frontend, Operation::ImportConvert, family, Platform::Native);
    request.extension = path.extension().and_then(|v| v.to_str());
    request.variant = Some(variant);
    match registry::admission(request) {
        registry::Decision::Allowed
        | registry::Decision::AllowedWithPreflight(registry::Reason::ConvertRequiresConsent) => {
            Ok(())
        }
        decision => decision.require_allowed(request),
    }
}
pub(super) fn check_losses(values: &[String]) -> Result<()> {
    if values.iter().any(|v| {
        !float_import::KNOWN_LOSSES.contains(&v.as_str())
            && !precision::KNOWN_LOSSES.contains(&v.as_str())
    }) {
        return Err(Error::new(
            ErrorCode::InputSchema,
            "Unknown float import loss acknowledgement",
        ));
    }
    Ok(())
}
pub(super) struct Prepared {
    value: wire::native::PreparedImport,
    pub input: PathBuf,
    identity: SourceIdentity,
    pub length: u64,
    pub owner: wire::Owner,
    pub selection: Selection,
    cancel: paintcraft_runtime::Cancellation,
    host_check: Check,
    // Retains the transient quote through detached receipt/staged output work.
    _lease: ResourceLease,
    _receipt: BudgetAccount,
}
impl Prepared {
    pub fn check(&self) -> Result<()> {
        self.cancel.check()?;
        (self.host_check)()
    }
    pub fn inspection(&self) -> Result<Value> {
        self.check()?;
        let assessment = self
            .value
            .float_assessment()
            .ok_or_else(|| Error::new(ErrorCode::InvariantViolation, "Missing float assessment"))?;
        let settings =
            paintcraft_core::types::json::parse(&serde_json::to_vec(&self.selection).map_err(
                |_| Error::new(ErrorCode::InputSchema, "Cannot serialize float settings"),
            )?)
            .map_err(|_| {
                Error::new(
                    ErrorCode::InvariantViolation,
                    "Cannot parse finite float settings",
                )
            })?;
        let value = Value::object([
            ("schema", Value::str("paintcraft.float-inspection/v1")),
            ("assessment_only", Value::Bool(true)),
            (
                "source_sha256",
                Value::str(hex(&assessment.binding().source_sha256())),
            ),
            ("assessment_sha256", Value::str(hex(&assessment.digest()))),
            ("decoder_profile", Value::str(self.value.decoder_profile())),
            ("settings", settings),
            ("source_assessment", assessment.to_value()),
            ("preserving_save", Value::Bool(false)),
        ]);
        self.identity.verify_bytes(
            &self.input,
            self.length,
            &assessment.binding().source_sha256(),
            &|| self.check(),
        )?;
        self.check()?;
        Ok(value)
    }
    pub fn adopt(self, expected: [u8; 32], allowed: &[String]) -> Result<Adopted> {
        check_losses(allowed)?;
        self.check()?;
        let check = || {
            self.cancel.check()?;
            (self.host_check)()
        };
        let value = self
            .value
            .adopt_float(expected, allowed, &check, &|| Ok(self.owner))?;
        Ok(Adopted {
            value,
            length: self.length,
            owner: self.owner,
            cancel: self.cancel,
            host_check: self.host_check,
            _lease: self._lease,
            _receipt: self._receipt,
        })
    }
}
pub(super) struct Adopted {
    pub value: wire::native::AdoptedImport,
    pub length: u64,
    pub owner: wire::Owner,
    cancel: paintcraft_runtime::Cancellation,
    host_check: Check,
    _lease: ResourceLease,
    _receipt: BudgetAccount,
}
impl Adopted {
    pub fn check(&self) -> Result<()> {
        self.cancel.check()?;
        (self.host_check)()
    }
    pub fn protect(&self, destination: &Path) -> Result<()> {
        self.check()?;
        if self.value.identity.matches_path(destination)? {
            return Err(Error::new(
                ErrorCode::InputSchema,
                "Float conversion must not replace its captured source",
            ));
        }
        self.value.identity.verify_bytes(
            &self.value.input,
            self.length,
            &self.value.sha256,
            &|| self.check(),
        )
    }
}
#[allow(clippy::too_many_arguments)] // Host ownership, settings, admission and runtime budget are separate contracts.
pub(super) fn prepare(
    path: &Path,
    selection: Selection,
    expected: Option<String>,
    owner: wire::Owner,
    pool: SharedBudget,
    host_check: Check,
    frontend: Frontend,
) -> Result<Prepared> {
    let settings = selection.settings()?;
    if let Some(expected) = &expected {
        digest(expected)?;
    }
    host_check()?;
    let (_, codec, _) = profile(path)?;
    admit(path, frontend, codec)?;
    let identity = SourceIdentity::existing_destination(path)?.ok_or_else(|| {
        Error::new(
            ErrorCode::StorageFailed,
            "Float source must be a regular file",
        )
    })?;
    let length = identity
        .file()
        .metadata()
        .map_err(|e| storage("float source metadata", &e))?
        .len();
    let quote = wire::native::float_reservation_bytes(length)?;
    let retained = identity.clone();
    let input = path.to_owned();
    let worker_input = input.clone();
    let worker_check = host_check.clone();
    let metadata = SharedBudget::new(METADATA_BYTES);
    // A 64 KiB header becomes 128 KiB hex; this covers bounded detached JSON/canonical copies.
    let mut receipt = metadata.account(AllocationClass::PaintMetadata);
    receipt.resize(1 << 20)?;
    let mut executor = Executor::with_budget(
        Limits {
            workers: 1,
            pending_jobs: 1,
            transient_bytes: 512 << 20,
        },
        pool,
    )?;
    let handle = executor.submit(
        quote,
        move |cancel| {
            let lease = cancel.resource_lease().ok_or_else(|| {
                Error::new(ErrorCode::ResourceLimit, "Float admission lease missing")
            })?;
            let check = || {
                cancel.check()?;
                worker_check()
            };
            let source = worker_input.clone();
            let job = wire::native::Job {
                input: worker_input,
                identity,
                length,
                codec,
                owner,
                pixels: wire::float::PIXELS_MAX,
                deadline: Duration::from_secs(10),
                application_version: env!("CARGO_PKG_VERSION").into(),
                expected_sha256: expected,
            };
            let value = job.prepare_float(
                settings,
                SharedBudget::new(wire::native::float_max_resident_bytes()?),
                metadata,
                lease.clone(),
                &check,
                &|| Ok(owner),
                |selected, _| admit(&source, frontend, selected),
                super::codec_worker::launch,
            )?;
            Ok((value, cancel, lease))
        },
        || {},
    )?;
    let mut cancelled = None;
    loop {
        if cancelled.is_none()
            && let Err(error) = host_check()
        {
            executor.cancel(handle);
            cancelled = Some(error);
        }
        if let Some(result) = executor.poll_for(handle) {
            if let Some(error) = cancelled {
                return Err(error);
            }
            let (value, cancel, lease) = result?;
            host_check()?;
            return Ok(Prepared {
                value,
                input,
                identity: retained,
                length,
                owner,
                selection,
                cancel,
                host_check,
                _lease: lease,
                _receipt: receipt,
            });
        }
        std::thread::sleep(Duration::from_millis(2));
    }
}

#[cfg(test)]
mod tests;
