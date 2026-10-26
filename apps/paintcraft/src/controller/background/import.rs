//! File conversion shares editing admission and keeps completion ownership in App.
use super::*;
use paintcraft_formats::codec_worker::native::PreparedImport;

#[derive(Debug)]
pub(crate) struct ImportPending(Pending);
impl ImportPending {
    pub(crate) fn poll(&self) -> Option<Result<PreparedImport>> {
        self.0.poll().map(|result| {
            result.and_then(|completion| match completion {
                Completion::CodecImport(prepared) => Ok(*prepared),
                _ => Err(Error::new(
                    ErrorCode::InvariantViolation,
                    "Unexpected image import completion",
                )),
            })
        })
    }
}
impl Service {
    pub(crate) fn import_residency(&self) -> paintcraft_runtime::SharedBudget {
        self.residency.clone()
    }
    /// Existing Pending drop cancels/abandons its exact handle. The executor
    /// retains the preparation charge until worker completion/result retirement.
    pub(crate) fn submit_import(
        &self,
        task: impl FnOnce(Cancellation) -> Result<PreparedImport> + Send + 'static,
    ) -> Result<ImportPending> {
        self.submit(
            move |cancel| task(cancel).map(|prepared| Completion::CodecImport(Box::new(prepared))),
            "Import image".into(),
            "",
            None,
        )
        .map(ImportPending)
    }
}

impl Editor {
    pub(crate) fn native_import_owner(&self) -> paintcraft_formats::codec_worker::Owner {
        let ticket = self.session.history_ticket();
        paintcraft_formats::codec_worker::Owner {
            session: ticket.owner,
            state: ticket.base.state_id,
            epoch: ticket.base.publication_epoch,
            frontend: 3,
        }
    }
}
