//! One narrowly admitted native JXL conversion. State is locked throughout;
//! subprocess/source/output work stays private until final owner/cancel checks.
use super::{
    super::*,
    state::{State, reply},
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

pub(super) const VARIANT: &str = wire::NATIVE_JXL_IMPORT_VARIANT;
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
fn owner(state: &State, empty: wire::Owner) -> wire::Owner {
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
fn admit(path: &Path) -> Result<()> {
    use paintcraft_formats::registry::{self, Family, Frontend, Operation, Platform, Request};
    let mut request = Request::new(
        Frontend::Studio,
        Operation::ImportConvert,
        Family::Jxl,
        Platform::Native,
    );
    request.extension = path.extension().and_then(|value| value.to_str());
    request.variant = Some(VARIANT);
    let decision = registry::admission(request);
    if decision.available() {
        Ok(())
    } else {
        decision.require_allowed(request)
    }
}
type Launch =
    Box<dyn FnOnce(wire::native::Launch<'_>, &dyn Fn() -> Result<()>) -> Result<Vec<u8>> + Send>;
fn preparation_budget() -> SharedBudget {
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
            Job {
                input,
                identity,
                length,
                codec: wire::Codec::JxlRgba8,
                owner: captured,
                pixels: wire::PIXELS_MAX,
                deadline: Duration::from_secs(10),
                application_version: env!("CARGO_PKG_VERSION").into(),
                expected_sha256: None,
            }
            .prepare_jxl(residency, lease, &check, &|| Ok(captured), launch)
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
    check: Check,
) -> Result<rmcp::model::CallToolResult> {
    import_with_launch(state, path, check, Box::new(launch))
}
fn import_with_launch(
    state: &mut State,
    path: &Path,
    check: Check,
    launch: Launch,
) -> Result<rmcp::model::CallToolResult> {
    admit(path)?;
    let empty = wire::Owner {
        session: uuid::Uuid::new_v4(),
        state: uuid::Uuid::new_v4(),
        epoch: 0,
        frontend: 3,
    };
    let captured = owner(state, empty);
    let prepared = prepare(path, captured, check.clone(), launch)?;
    let adopted = prepared.adopt(&*check, &|| Ok(owner(state, empty)))?;
    let mut session = Session::new(adopted.document);
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
    state.import_source = Some(adopted.input);
    state.import_identity = Some(adopted.identity);
    state.import_resources.clear();
    state.brush_sources.clear();
    let mut value = state.inspection()?;
    value["conversion"] = serde_json::json!({"source_sha256":hex(&adopted.sha256),"source_format":"jxl",
        "decoder_profile":VARIANT,"source_lineage_adopted":false,"preserving_save":false,"losses_acknowledged":[]});
    Ok(reply(value))
}

#[cfg(test)]
mod tests;
