//! Exact negative metadata, consumed before any App/editor poll.
use super::*;
use paintcraft_ui::pen_input::NativePenCancellation;
impl Host {
    pub(super) fn refresh_retirements(&mut self) -> bool {
        let installation = self
            .ingress
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .source_epoch();
        let drain = || {
            self.ingress
                .lock()
                .unwrap_or_else(|e| e.into_inner())
                .drain_retirements(&mut self.retirements)
                .is_ok()
        };
        if let Some(delivery) = &self.delivery {
            match installation {
                Some(epoch) => delivery.authorize_installation(epoch, drain),
                None => delivery.authorize(drain),
            }
        } else {
            #[cfg(test)]
            {
                let mut drain = drain;
                drain()
            }
            #[cfg(not(test))]
            {
                false
            }
        }
    }
    pub(super) fn apply_retirement(
        &mut self,
        ctx: &egui::Context,
        cancellation: &NativePenCancellation,
    ) {
        self.ingress
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .discard_retired_entries(cancellation);
        if self
            .waiting
            .as_ref()
            .is_some_and(|barrier| cancellation.matches_publication(&barrier.publication))
        {
            self.cancel_own_work();
            self.waiting = None;
        }
        if let Some(frame) = &mut self.pending {
            for entry in &mut frame.entries[self.cursor..] {
                if matches!(&entry.pointer, Pointer::Pen(batch) if cancellation.matches_batch(batch))
                {
                    entry.pointer = Pointer::Retired;
                }
            }
        }
        if self.app.retire_owned_contact(cancellation) {
            ctx.input_mut(|i| i.pointer = Default::default());
        }
    }
    pub(super) fn apply_retirements(&mut self, ctx: &egui::Context) {
        // Move one retained descriptor at a time; no allocation or global reset.
        while !self.retirements.is_empty() {
            let cancellation = self.retirements.remove(0);
            self.apply_retirement(ctx, &cancellation);
        }
    }
}
