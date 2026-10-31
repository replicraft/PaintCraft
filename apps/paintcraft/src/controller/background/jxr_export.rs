//! JPEG XR shares the editing executor and retains its exact admitted result.
use super::*;
use paintcraft_formats::codec_worker::native::PreparedJxrEncode;

#[derive(Debug)]
pub(crate) struct PreparedJxrExport {
    pub(crate) encoded: PreparedJxrEncode,
    pub(crate) losses: Vec<String>,
    pub(crate) preview: Option<egui::ColorImage>,
}
#[derive(Debug)]
pub(crate) struct JxrExportPending(Pending);
impl JxrExportPending {
    pub(crate) fn poll(&self) -> Option<Result<PreparedJxrExport>> {
        self.0.poll().map(|result| {
            result.and_then(|completion| match completion {
                Completion::JxrExport(prepared) => Ok(*prepared),
                _ => Err(Error::new(
                    ErrorCode::InvariantViolation,
                    "Unexpected JPEG XR export result",
                )),
            })
        })
    }
}
impl Service {
    /// Retry only executor-owned active contention; uncertain retirement is not
    /// reclamation, and external/impossible admission remains a typed refusal.
    pub(crate) fn try_submit_jxr_export(
        &self,
        quote: u64,
        task: impl FnOnce(Cancellation) -> Result<PreparedJxrExport> + Send + 'static,
    ) -> Result<Option<JxrExportPending>> {
        self.try_submit_jxr_export_with_workspace(quote, task, false)
    }
    pub(crate) fn try_submit_jxr_export_with_workspace(
        &self,
        quote: u64,
        task: impl FnOnce(Cancellation) -> Result<PreparedJxrExport> + Send + 'static,
        spatial: bool,
    ) -> Result<Option<JxrExportPending>> {
        if quote > PREPARATION_CHARGE {
            return Err(Error::new(
                ErrorCode::ResourceLimit,
                "JPEG XR export exceeds editing admission",
            ));
        }
        let submitted = if spatial {
            self.submit_jxr_export_with_workspace(quote, task, true)
        } else {
            self.submit_jxr_export(quote, task)
        };
        match submitted {
            Ok(pending) => Ok(Some(pending)),
            Err(error) if error.code == ErrorCode::ResourceLimit => {
                let mut executor = self.lock()?;
                executor.reap_abandoned();
                if executor.request_fits(
                    PREPARATION_CHARGE
                        + if spatial {
                            crate::spatial_workspace::BYTES
                        } else {
                            0
                        },
                ) && executor.pending() > 0
                {
                    Ok(None)
                } else {
                    Err(error)
                }
            }
            Err(error) => Err(error),
        }
    }
    pub(crate) fn submit_jxr_export(
        &self,
        quote: u64,
        task: impl FnOnce(Cancellation) -> Result<PreparedJxrExport> + Send + 'static,
    ) -> Result<JxrExportPending> {
        self.submit_jxr_export_with_workspace(quote, task, false)
    }
    pub(crate) fn submit_jxr_export_with_workspace(
        &self,
        quote: u64,
        task: impl FnOnce(Cancellation) -> Result<PreparedJxrExport> + Send + 'static,
        spatial: bool,
    ) -> Result<JxrExportPending> {
        if quote > PREPARATION_CHARGE {
            return Err(Error::new(
                ErrorCode::ResourceLimit,
                "JPEG XR export exceeds editing admission",
            ));
        }
        self.submit_with_workspace(
            move |cancel| task(cancel).map(|value| Completion::JxrExport(Box::new(value))),
            "Export JPEG XR".into(),
            "",
            None,
            spatial,
        )
        .map(JxrExportPending)
    }
}

#[cfg(test)]
#[path = "jxr_export/tests.rs"]
mod tests;
