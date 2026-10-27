//! Captured plain-round pen ink uses the existing edit service and publication.
use super::*;
use paintcraft_core::types::{Error, ErrorCode, resources::SharedBudget};
use paintcraft_editor::session::{Preparation, PreparedEdit, Receipt, Ticket};
use paintcraft_input::tablet::{Phase, Sample};
use paintcraft_tools::pen_stroke::{PenStroke, ResolvedPenStroke};

#[derive(Debug)]
pub(super) struct Prepared {
    edit: PreparedEdit,
    source: PenStroke,
    mask: ResolvedPenStroke,
}
impl Prepared {
    pub(super) fn publish(self, session: &mut Session) -> Result<Receipt> {
        let Self { edit, source, mask } = self;
        let result = session.publish_prepared(edit, || Ok(()));
        drop((source, mask));
        result
    }
}
fn prepare(
    source: PenStroke,
    preparation: Preparation,
    ticket: Ticket,
    clip: paintcraft_core::types::Rect,
    admission: Option<paintcraft_raster::Admission>,
    check: &dyn Fn() -> Result<()>,
) -> Result<Prepared> {
    let mask = source.resolve_captured(clip, check)?;
    let operations = mask.operations(&check)?;
    let edit = if operations.is_empty() {
        preparation.no_change(check)?
    } else {
        let batch = strokes::captured_batch(ticket, operations)?;
        match admission {
            Some(admission) => preparation.apply_pen_mask_stroke_with_admission(
                &batch,
                admission,
                source.style().intent,
                check,
            )?,
            None => preparation.apply_pen_mask_stroke(&batch, source.style().intent, check)?,
        }
    };
    Ok(Prepared { edit, source, mask })
}
impl Editor {
    #[cfg(target_os = "macos")]
    pub(crate) fn pressure_eligible(&self, sample: Sample) -> bool {
        self.tools.pressure_eligible(sample)
    }
    /// True means the dedicated pressure path owns this sample. False preserves
    /// the established source-specific tool route; callers must not relabel media.
    pub(crate) fn pen_sample(&mut self, sample: Sample, budget: &SharedBudget) -> Result<bool> {
        if matches!(sample.phase, Phase::Cancel | Phase::Leave) {
            self.cancel_gesture();
            return Ok(true);
        }
        if sample.phase == Phase::Down {
            if self.painting_blocked()
                || self.is_editing_text()
                || self.has_image_paste()
                || self.has_floating_selection()
                || self.is_constructing_shape()
            {
                return Err(Error::new(
                    ErrorCode::TargetInvalid,
                    "Finish the current draft before pen painting",
                ));
            }
            if !self.tools.pressure_eligible(sample) {
                return Ok(false);
            }
            if self.has_pending_edits() {
                return Err(Error::new(
                    ErrorCode::ResourceLimit,
                    "Wait for the current edit, then start a new pen stroke",
                ));
            }
            let layer = self.target.ok_or_else(|| {
                Error::new(
                    ErrorCode::TargetInvalid,
                    "Choose a visible raster layer before pen painting",
                )
            })?;
            if !self
                .document()
                .layers
                .get(&layer)
                .is_some_and(|l| l.visible)
            {
                return Err(Error::new(
                    ErrorCode::TargetInvalid,
                    "Choose a visible raster layer before pen painting",
                ));
            }
            if self.tools.begin_pen(layer, sample, budget)? {
                self.pen_base = Some(self.session.ticket());
                return Ok(true);
            }
            return Ok(false);
        }
        if !self.tools.pen_in_progress() {
            return Ok(false);
        }
        if self.pen_base != Some(self.session.ticket()) {
            self.cancel_gesture();
            return Err(Error::new(
                ErrorCode::ConflictStaleBase,
                "Pen source document changed during capture",
            ));
        }
        if let Some((token, action)) = self.tools.append_pen(sample)? {
            self.pen_base = None;
            self.request(token, action);
        }
        Ok(true)
    }
    pub(super) fn pen_stroke(&mut self, stroke: PenStroke) {
        let ticket = self.session.ticket();
        let clip = self.document().canvas_rect();
        let done = match stroke.style().intent {
            paintcraft_core::types::policy::MaskIntent::Brush => "Brush",
            paintcraft_core::types::policy::MaskIntent::Pencil => "Pencil",
            paintcraft_core::types::policy::MaskIntent::Erase => "Erase",
        };
        if let Some(service) = &self.editing {
            let admission = service.admission(u64::MAX);
            let task = self.session.preparation().map(|preparation| {
                Box::new(move |cancel: paintcraft_runtime::Cancellation| {
                    prepare(stroke, preparation, ticket, clip, Some(admission), &|| {
                        cancel.check()
                    })
                    .map(background::Completion::PenEdit)
                }) as dispatch::Task
            });
            self.dispatch_initial(task, done.into(), "", None);
        } else {
            let result = self
                .session
                .preparation()
                .and_then(|preparation| {
                    prepare(stroke, preparation, ticket, clip, None, &|| Ok(()))
                })
                .and_then(|prepared| {
                    let before = self.session.ticket();
                    let receipt = prepared.publish(&mut self.session)?;
                    self.record_native_publication(before, &receipt);
                    Ok(receipt)
                });
            match result {
                Ok(receipt) => self.accept_edit(&receipt, done, None, self.document().canvas),
                Err(error) => self.status = format!("Pen stroke not prepared: {error}"),
            }
        }
    }
}

#[cfg(test)]
#[path = "pen/tests.rs"]
mod tests;
