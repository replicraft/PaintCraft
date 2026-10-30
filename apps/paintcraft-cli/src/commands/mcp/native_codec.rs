//! Narrowly admitted native JXL/JXR/HEIF/AVIF conversions. State is locked throughout;
//! subprocess/source/output work stays private until final owner/cancel checks.
use super::{
    super::*,
    state::{State, core_json, reply},
};
use paintcraft_formats::codec_worker::{
    self as wire,
    native::{Job, PreparedImport},
};
use paintcraft_platform::source::SourceIdentity;
use paintcraft_runtime::{Executor, Limits, SharedBudget, process};
use std::{
    sync::{
        Arc, OnceLock,
        atomic::{AtomicBool, Ordering},
    },
    time::Duration,
};

pub(super) type Check = Arc<dyn Fn() -> Result<()> + Send + Sync>;
/// Dropping an RPC future invalidates publication even if spawn_blocking keeps
/// running. The worker still drains cancellation/reap before releasing its lease.
pub(super) struct RpcLife(pub Arc<AtomicBool>);
impl RpcLife {
    pub fn new() -> Self {
        Self(Arc::new(AtomicBool::new(false)))
    }
}
impl Drop for RpcLife {
    fn drop(&mut self) {
        self.0.store(true, Ordering::Release);
    }
}
pub(super) fn owner(state: &State, empty: wire::Owner) -> wire::Owner {
    state.session.as_ref().map_or(empty, |session| {
        let history = session.history_ticket();
        wire::Owner {
            session: history.owner,
            state: history.base.state_id,
            epoch: history.base.publication_epoch,
            frontend: 3,
        }
    })
}
fn profile(
    path: &Path,
) -> Result<(
    paintcraft_formats::registry::Family,
    wire::Codec,
    &'static str,
)> {
    use paintcraft_formats::registry::Family;
    match path
        .extension()
        .and_then(|ext| ext.to_str())
        .and_then(Family::from_extension)
    {
        Some(Family::Jxl) => Ok((
            Family::Jxl,
            wire::Codec::JxlRgba8,
            wire::NATIVE_JXL_IMPORT_VARIANT,
        )),
        Some(Family::Jxr) => Ok((
            Family::Jxr,
            wire::Codec::JxrRgb8,
            wire::NATIVE_JXR_IMPORT_VARIANT,
        )),
        Some(Family::Avif) => Ok((
            Family::Avif,
            wire::Codec::AvifSrgb8,
            wire::NATIVE_AVIF_IMPORT_VARIANT,
        )),
        Some(Family::Heif) => Ok((
            Family::Heif,
            wire::Codec::HeifSrgb8,
            wire::NATIVE_HEIF_IMPORT_VARIANT,
        )),
        _ => Err(Error::new(
            ErrorCode::CapabilityUnsupported,
            "Native codec import requires JXL/JXR/HEIF/AVIF",
        )),
    }
}
fn admit(
    path: &Path,
) -> Result<(
    paintcraft_formats::registry::Family,
    wire::Codec,
    &'static str,
)> {
    use paintcraft_formats::registry::{self, Family, Frontend, Operation, Platform, Request};
    let (family, codec, variant) = profile(path)?;
    let mut request = Request::new(
        Frontend::Studio,
        Operation::ImportConvert,
        family,
        Platform::Native,
    );
    request.extension = path.extension().and_then(|ext| ext.to_str());
    request.variant = Some(variant);
    match registry::admission(request) {
        registry::Decision::Allowed => {}
        registry::Decision::AllowedWithPreflight(registry::Reason::ConvertRequiresConsent)
            if matches!(family, Family::Jxr | Family::Heif | Family::Avif) => {}
        decision => decision.require_allowed(request)?,
    }
    Ok((family, codec, variant))
}

type Launch =
    Box<dyn FnOnce(wire::native::Launch<'_>, &dyn Fn() -> Result<()>) -> Result<Vec<u8>> + Send>;
pub(super) fn preparation_budget() -> SharedBudget {
    static PREPARATION: OnceLock<SharedBudget> = OnceLock::new();
    PREPARATION
        .get_or_init(|| SharedBudget::new(512 << 20))
        .clone()
}
fn launch(launch: wire::native::Launch<'_>, check: &dyn Fn() -> Result<()>) -> Result<Vec<u8>> {
    process::run_parent_lease(
        launch.executable,
        launch.arguments,
        launch.request.segments(),
        process::Limits {
            input_bytes: wire::INPUT_BYTES_MAX + wire::REQUEST_HEADER,
            output_bytes: wire::OUTPUT_BYTES_MAX,
            deadline: launch.remaining,
            resource_exit_code: Some(wire::RESOURCE_EXIT),
        },
        check,
    )
    .map(|output| output.bytes)
}
fn prepare(
    path: &Path,
    captured: wire::Owner,
    codec: wire::Codec,
    check: Check,
    launch: Launch,
) -> Result<PreparedImport> {
    check()?;
    let identity = SourceIdentity::existing_destination(path)?.ok_or_else(|| {
        Error::new(
            ErrorCode::StorageFailed,
            "Native codec input must be a regular file",
        )
    })?;
    let length = identity
        .file()
        .metadata()
        .map_err(|e| storage("codec metadata", &e))?
        .len();
    // Maximum padded JXL document 64MiB, index/writer 17MiB and IO 1MiB.
    let reservation = wire::reservation_bytes_for_input(length)? + (82 << 20);
    let budget = preparation_budget();
    let residency = paintcraft_raster::Admission::new(0).residency_budget();
    let mut executor = Executor::with_budget(
        Limits {
            workers: 1,
            pending_jobs: 1,
            transient_bytes: 512 << 20,
        },
        budget,
    )?;
    let input = path.to_owned();
    let worker_check = check.clone();
    let handle = executor.submit(
        reservation,
        move |cancel| {
            let lease = cancel.resource_lease().ok_or_else(|| {
                Error::new(ErrorCode::ResourceLimit, "Native codec admission missing")
            })?;
            let check = || {
                cancel.check()?;
                worker_check()
            };
            let source = input.clone();
            let job = Job {
                input,
                identity,
                length,
                codec,
                owner: captured,
                pixels: wire::PIXELS_MAX,
                deadline: Duration::from_secs(10),
                application_version: env!("CARGO_PKG_VERSION").into(),
                expected_sha256: None,
            };
            if codec == wire::Codec::JxlRgba8 {
                job.prepare_jxl_family(
                    residency,
                    lease,
                    &check,
                    &|| Ok(captured),
                    |selected| {
                        super::super::format_profile::Options::default()
                            .admit_import_jxl_codec(&source, selected)
                    },
                    launch,
                )
            } else if codec == wire::Codec::JxrRgb8 {
                job.prepare_jxr_family(
                    residency,
                    lease,
                    &check,
                    &|| Ok(captured),
                    |assessment| {
                        super::super::format_profile::Options::default()
                            .admit_import_jxr_assessment(&source, assessment)
                    },
                    launch,
                )
            } else if codec == wire::Codec::AvifSrgb8 {
                job.prepare_avif_family(
                    residency,
                    lease,
                    &check,
                    &|| Ok(captured),
                    |selected, _| {
                        super::super::format_profile::Options::default()
                            .admit_import_avif_codec(&source, selected)
                    },
                    launch,
                )
            } else {
                job.prepare_import(residency, lease, &check, &|| Ok(captured), launch)
            }
        },
        || {},
    )?;
    let mut cancelled = None;
    loop {
        if cancelled.is_none()
            && let Err(error) = check()
        {
            executor.cancel(handle);
            cancelled = Some(error);
        }
        if let Some(result) = executor.poll_for(handle) {
            if let Some(error) = cancelled {
                return Err(error);
            }
            return result;
        }
        std::thread::sleep(Duration::from_millis(2));
    }
}
pub(super) fn import(
    state: &mut State,
    path: &Path,
    allowed: &[String],
    check: Check,
) -> Result<rmcp::model::CallToolResult> {
    import_with_launch(state, path, allowed, check, Box::new(launch))
}
fn import_with_launch(
    state: &mut State,
    path: &Path,
    allowed: &[String],
    check: Check,
    launch: Launch,
) -> Result<rmcp::model::CallToolResult> {
    let (family, codec, _) = admit(path)?;
    let empty = wire::Owner {
        session: uuid::Uuid::new_v4(),
        state: uuid::Uuid::new_v4(),
        epoch: 0,
        frontend: 3,
    };
    let captured = owner(state, empty);
    let prepared = prepare(path, captured, codec, check.clone(), launch)?;
    let decoder_profile = prepared.decoder_profile();
    let actual_losses: Vec<String> = prepared
        .losses()
        .iter()
        .map(|code| (*code).to_owned())
        .collect();
    let jxl_digest = if prepared.jxl_integer_assessment().is_some() {
        Some(prepared.jxl_integer_assessment_digest()?)
    } else {
        None
    };
    let assessment = prepared
        .jxr_assessment()
        .map(|assessment| assessment.to_value())
        .or_else(|| {
            prepared
                .heif_assessment()
                .map(|assessment| assessment.to_value())
        })
        .or_else(|| {
            prepared
                .avif_assessment()
                .map(|assessment| assessment.to_value())
        })
        .or_else(|| {
            prepared
                .avif_sdr_assessment()
                .map(|assessment| assessment.to_value())
        })
        .or_else(|| {
            prepared
                .jxl_integer_assessment()
                .zip(jxl_digest.as_ref())
                .map(|(report, digest)| {
                    super::super::codec_import::jxl_assessment_value(report, digest)
                })
        })
        .map(|value| core_json(&value))
        .transpose()?;
    let adopted = if let Some(digest) = jxl_digest {
        prepared.adopt_jxl_integer(digest, allowed, &*check, &|| Ok(owner(state, empty)))?
    } else {
        prepared.adopt_with_losses(allowed, &*check, &|| Ok(owner(state, empty)))?
    };
    let mut session = super::super::persistence::session(adopted.document);
    session.mark_unsaved();
    check()?;
    if owner(state, empty) != captured {
        return Err(Error::new(
            ErrorCode::ConflictStaleBase,
            "MCP document changed during codec import",
        ));
    }
    state.session = Some(session);
    state.source = None;
    state.project_identity = None;
    state.import_source = Some(adopted.input);
    state.import_identity = Some(adopted.identity);
    state.import_resources.clear();
    state.brush_sources.clear();
    let mut value = state.inspection()?;
    value["conversion"] = serde_json::json!({"source_sha256":hex(&adopted.sha256),"source_format":family.id(),
        "decoder_profile":decoder_profile,"source_lineage_adopted":false,"preserving_save":false,"losses_acknowledged":actual_losses,"source_assessment":assessment});
    Ok(reply(value))
}

#[cfg(test)]
mod tests;
