//! Native output uses the platform coordinator's post-staging checkpoint.
use super::*;
pub(super) fn write_native(
    dest: &Path,
    overwrite: bool,
    document: &Document,
    ctx: &Context<'_>,
) -> (Value, bool, String) {
    write_native_guarded(dest, overwrite, document, ctx, || Ok(()))
}
/// Checks run after synchronized staging and before the final fingerprint/replacement.
/// A namespace can still change after this checkpoint; this is not a filesystem CAS.
pub(super) fn write_native_guarded(
    dest: &Path,
    overwrite: bool,
    document: &Document,
    ctx: &Context<'_>,
    check_sources: impl FnOnce() -> Result<()>,
) -> (Value, bool, String) {
    let encode = |sink: &mut dyn std::io::Write| pcnd::write_compact_to(document, sink);
    let prepare = |_: &Outcome| {
        // Tests mutate the namespace at this actual checkpoint, after staging.
        #[cfg(test)]
        CHECKPOINT.with(|slot| {
            if let Some(inject) = slot.borrow_mut().take() {
                inject();
            }
        });
        check_sources()
    };
    let outcome = if ctx.operation == "save" {
        match ctx.source_sha256.as_deref() {
            Some(expected) => save::save_with_expected_sha256_and_preparation(
                dest,
                overwrite,
                save::STAGED_FILE_BYTES_MAX,
                expected,
                encode,
                prepare,
            ),
            None => save::not_attempted(&Error::new(
                ErrorCode::ConflictStaleBase,
                "in-place output requires the captured source digest",
            )),
        }
    } else {
        save::save_with_preparation(
            dest,
            overwrite,
            save::STAGED_FILE_BYTES_MAX,
            encode,
            prepare,
        )
    };
    finish_output(dest, ctx, outcome)
}
#[cfg(test)]
thread_local! {
    static CHECKPOINT: std::cell::RefCell<Option<Box<dyn FnOnce()>>> = const { std::cell::RefCell::new(None) };
}
#[cfg(test)]
pub(super) fn inject_at_checkpoint(inject: impl FnOnce() + 'static, run: impl FnOnce()) {
    struct Reset;
    impl Drop for Reset {
        fn drop(&mut self) {
            CHECKPOINT.with(|slot| {
                slot.borrow_mut().take();
            });
        }
    }
    CHECKPOINT.with(|slot| assert!(slot.borrow_mut().replace(Box::new(inject)).is_none()));
    let _reset = Reset;
    run();
}
#[cfg(test)]
mod tests;
