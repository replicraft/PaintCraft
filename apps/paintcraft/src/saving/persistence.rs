//! Snapshot/receipt persistence remains on the same admitted native I/O worker.
use super::*;
use paintcraft_formats::save_recovery::{SavedAttempt, Stamp};

#[derive(Debug)]
pub(super) struct FileResult {
    pub outcome: save::Outcome,
    pub retention: Option<Arc<SavedAttempt>>,
}

pub(super) fn save_document(
    document: &paintcraft_document::Document,
    destination: &Path,
    recovery_directory: Option<&Path>,
    stamp: Stamp,
) -> Result<Completion> {
    let retention = match recovery_directory {
        Some(directory) => match SavedAttempt::capture(directory, document, destination, stamp) {
            Ok(retained) => Some(Arc::new(retained)),
            Err(error) => return Ok(Completion::File(save::not_attempted(&error))),
        },
        None => None, // Deterministic/embedded harness without configured recovery.
    };
    let mut outcome = save::save_with_preparation(
        destination,
        true,
        save::STAGED_FILE_BYTES_MAX,
        |sink| paintcraft_formats::pcnd::write_to(document, sink),
        |outcome| {
            retention
                .as_ref()
                .map_or(Ok(()), |retained| retained.prepare(outcome))
        },
    );
    if let Some(retained) = &retention
        && let Err(error) = retained.finish(&outcome)
    {
        // Destination success and receipt persistence are separate facts.
        outcome
            .reasons
            .push(format!("Recovery evidence update failed: {error}"));
    }
    Ok(Completion::RetainedFile(Box::new(FileResult {
        outcome,
        retention,
    })))
}

impl NativeSaving {
    pub(super) fn submit_io_with_ticket(
        &mut self,
        bytes: u64,
        work: impl FnOnce(Ticket) -> Result<Completion> + Send + 'static,
        wake: impl FnOnce() + Send + 'static,
    ) -> Result<Ticket> {
        self.admit(1, bytes)?;
        self.io.submit_with_ticket(bytes, work, wake)
    }
    pub(super) fn pump_retired_saves(&mut self) {
        while let Some(retained) = self.retired_saves.last().cloned() {
            let context = self.context.clone();
            match self.submit_io(
                64 * 1024,
                move || {
                    retained.discard()?;
                    Ok(Completion::Cleanup)
                },
                move || context.request_repaint(),
            ) {
                Ok(ticket) => {
                    self.retired_saves.pop();
                    self.pending.push(Pending {
                        ticket,
                        kind: PendingKind::Cleanup,
                    });
                }
                Err(error) => {
                    if error.code != ErrorCode::ResourceLimit {
                        self.failed = true;
                    }
                    break;
                }
            }
        }
    }
}

#[cfg(test)]
mod tests;
