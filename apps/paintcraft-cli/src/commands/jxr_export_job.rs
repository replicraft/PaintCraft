//! Shared native encoding preparation. Caller retains source/publication guards.
use super::*;
use paintcraft_formats::codec_worker::{self as wire, jxr_encode as encode};
use paintcraft_runtime::{Executor, Limits, SharedBudget, process};
use std::{
    sync::Arc,
    time::{Duration, Instant},
};
pub(super) type Check = Arc<dyn Fn() -> Result<()> + Send + Sync>;
const DEADLINE: Duration = Duration::from_secs(10);
pub(super) struct Prepared {
    pub encoded: wire::native::PreparedJxrEncode,
    pub losses: Vec<String>,
    cancel: paintcraft_runtime::Cancellation,
    check: Check,
    started: Instant,
}
impl Prepared {
    pub fn check(&self, current: &dyn Fn() -> Result<wire::Owner>) -> Result<()> {
        self.cancel.check()?;
        (self.check)()?;
        if self.started.elapsed() >= DEADLINE {
            return Err(Error::new(
                ErrorCode::ResourceLimit,
                "JXR export deadline exceeded",
            ));
        }
        self.encoded.check_current(
            &|| {
                self.cancel.check()?;
                (self.check)()
            },
            current,
        )
    }
}
pub(super) fn prepare(
    doc: Document,
    owner: wire::Owner,
    pool: SharedBudget,
    host: Check,
    allowed: Vec<String>,
) -> Result<Prepared> {
    host()?;
    let layout = encode::Layout::new(doc.canvas.0, doc.canvas.1, encode::ENCODED_BYTES_MAX)?;
    let rgba = u64::from(doc.canvas.0) * u64::from(doc.canvas.1) * 4;
    let quote = layout
        .reservation_bytes()?
        .checked_add(rgba)
        .ok_or_else(|| Error::new(ErrorCode::ResourceLimit, "JXR export reservation overflow"))?;
    let mut executor = Executor::with_budget(
        Limits {
            workers: 1,
            pending_jobs: 1,
            transient_bytes: 512 << 20,
        },
        pool,
    )?;
    let started = Instant::now();
    let worker_check = host.clone();
    let handle = executor.submit(
        quote,
        move |cancel| {
            let lease = cancel.resource_lease().ok_or_else(|| {
                Error::new(
                    ErrorCode::ResourceLimit,
                    "JXR export admission lease missing",
                )
            })?;
            let check = || {
                cancel.check()?;
                worker_check()?;
                if started.elapsed() >= DEADLINE {
                    return Err(Error::new(
                        ErrorCode::ResourceLimit,
                        "JXR export deadline exceeded",
                    ));
                }
                Ok(())
            };
            let prepared = export::jxr::prepare_opaque_rgb_checked(
                &doc,
                encode::ENCODED_BYTES_MAX,
                &allowed,
                |observed, composite| {
                    if observed == layout && composite == rgba {
                        Ok(())
                    } else {
                        Err(Error::new(
                            ErrorCode::InvariantViolation,
                            "JXR preparation quote changed",
                        ))
                    }
                },
                |doc, check| paintcraft_render::render_cancellable(doc, check),
                &check,
            )?;
            let (layout, rgb, losses) = prepared.into_parts();
            let remaining = DEADLINE
                .checked_sub(started.elapsed())
                .filter(|d| !d.is_zero())
                .ok_or_else(|| {
                    Error::new(ErrorCode::ResourceLimit, "JXR export deadline exceeded")
                })?;
            let encoded = wire::native::JxrEncodeJob {
                layout,
                owner,
                deadline: remaining,
            }
            .prepare(
                Arc::new(rgb),
                lease,
                &check,
                &|| Ok(owner),
                |_, _| Ok(()),
                launch,
            )?;
            Ok((encoded, losses, cancel))
        },
        || {},
    )?;
    let mut cancelled = None;
    loop {
        if cancelled.is_none()
            && let Err(error) = host()
        {
            executor.cancel(handle);
            cancelled = Some(error);
        }
        if let Some(result) = executor.poll_for(handle) {
            if let Some(error) = cancelled {
                return Err(error);
            }
            let (encoded, losses, cancel) = result?;
            host()?;
            return Ok(Prepared {
                encoded,
                losses,
                cancel,
                check: host,
                started,
            });
        }
        std::thread::sleep(Duration::from_millis(2));
    }
}
fn launch(
    context: wire::native::JxrEncodeLaunch<'_>,
    check: &dyn Fn() -> Result<()>,
) -> Result<Vec<u8>> {
    process::run_parent_lease(
        context.executable,
        context.arguments,
        context.request.segments(),
        process::Limits {
            input_bytes: encode::REQUEST_HEADER + encode::RGB_BYTES_MAX as usize,
            output_bytes: context.request.header().layout().response_limit()? as usize,
            deadline: context.remaining,
            resource_exit_code: Some(wire::RESOURCE_EXIT),
        },
        check,
    )
    .map(|result| result.bytes)
}
