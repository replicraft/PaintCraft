//! Existing App tablet consumer handed one actual owned-loop edge at a time.
use super::*;
impl App {
    pub(crate) fn connect_owned_tablet(
        &mut self,
        source: Source,
        budget: &SharedBudget,
    ) -> paintcraft_core::types::Result<()> {
        if self.tablet.is_some() {
            return Err(paintcraft_core::types::Error::new(
                paintcraft_core::types::ErrorCode::TargetInvalid,
                "Retire existing tablet installation first",
            ));
        }
        self.tablet = Some(Receiver::new(source, None, budget)?);
        Ok(())
    }
    pub(crate) fn owned_owner(&self) -> Owner {
        self.tablet_owner()
    }
    pub(crate) fn begin_owned_edge(
        &mut self,
        batch: Option<Batch>,
        own_wait: bool,
    ) -> paintcraft_core::types::Result<()> {
        let owner = self.tablet_owner();
        let receiver = self.tablet.as_mut().expect("owned tablet receiver");
        if receiver.pending.is_some() {
            return Err(paintcraft_core::types::Error::new(
                paintcraft_core::types::ErrorCode::ResourceLimit,
                "Prior owned edge remains unconsumed",
            ));
        }
        receiver.frame = receiver.frame.checked_add(1).ok_or_else(|| {
            paintcraft_core::types::Error::new(
                paintcraft_core::types::ErrorCode::ResourceLimit,
                "Owned edge epoch exhausted",
            )
        })?;
        receiver.retained = None;
        receiver.defer_view = true;
        receiver.own_service_wait = own_wait;
        match batch {
            Some(batch) => {
                if !receiver.source.batch_is_current(&batch, owner) {
                    return Err(paintcraft_core::types::Error::new(
                        paintcraft_core::types::ErrorCode::ConflictStaleBase,
                        "Owned pen edge retired",
                    ));
                }
                receiver.pass.begin(receiver.frame, &batch.frame);
                receiver.pending = Some((owner, batch));
            }
            None => {
                receiver.pass.begin(receiver.frame, &Default::default());
            }
        }
        Ok(())
    }
    pub(crate) fn defer_owned_wait(&mut self, waiting: bool) {
        let receiver = self.tablet.as_mut().expect("owned tablet receiver");
        receiver.defer_view = true;
        receiver.own_service_wait = waiting;
    }
    pub(crate) fn finish_owned_view(&mut self) -> Option<CaptureView> {
        let receiver = self.tablet.as_mut()?;
        receiver.defer_view = false;
        receiver.own_service_wait = false;
        receiver.deferred_view.take().or(receiver.published_view)
    }
    /// Acquisition already retired Source at its ordered boundary. Replaying
    /// that boundary must not invalidate newer admitted batches behind it.
    pub(crate) fn retire_owned_presentation(&mut self) {
        if let Some(receiver) = &mut self.tablet {
            let owned = receiver.retire_presentation();
            receiver.defer_view = false;
            receiver.own_service_wait = false;
            if owned {
                self.editor.cancel_gesture();
            }
        }
    }
    pub(crate) fn retire_owned_tablet(&mut self) {
        if let Some(receiver) = &mut self.tablet {
            let owned = receiver.retire();
            receiver.defer_view = false;
            receiver.own_service_wait = false;
            if owned {
                self.editor.cancel_gesture();
            }
        }
    }
}
impl App {
    pub(crate) fn owned_deferred_view(&self) -> Option<CaptureView> {
        let receiver = self.tablet.as_ref()?;
        receiver.deferred_view.or(receiver.published_view)
    }
}
