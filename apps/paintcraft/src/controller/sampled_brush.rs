//! Sampled coverage prepares off the native UI thread, then publishes once.
use super::*;
impl Editor {
    pub(super) fn sampled_stroke(
        &mut self,
        stroke: paintcraft_tools::sampled_brush::SampledStroke,
    ) {
        let plan = self.prepare_preview(Some(Preview::SampledStroke(stroke)));
        let empty = self.document().layers.is_empty();
        if let Some(service) = &self.editing {
            let admission = service.admission(u64::MAX);
            let task = self.session.preparation().map(|preparation| {
                Box::new(move |cancel: paintcraft_runtime::Cancellation| {
                    let check = || cancel.check();
                    let prepared = match plan.build(check)? {
                        None => preparation.no_change(check)?,
                        Some(batch) if empty => preparation
                            .apply_initial_mask_stroke_with_admission(&batch, admission, check)?,
                        Some(batch) => preparation
                            .apply_mask_stroke_with_admission(&batch, admission, check)?,
                    };
                    Ok(background::Completion::Edit(prepared))
                }) as dispatch::Task
            });
            self.dispatch_initial(task, "Brush".into(), "", empty.then_some("sampled-first"));
        } else {
            let result: Result<()> = (|| {
                let Some(batch) = plan.build(|| Ok(()))? else {
                    return Ok(());
                };
                let preparation = self.session.preparation()?;
                let prepared = if empty {
                    preparation.apply_initial_mask_stroke(&batch, || Ok(()))?
                } else {
                    preparation.apply_mask_stroke(&batch, || Ok(()))?
                };
                let receipt = self.session.publish_prepared(prepared, || Ok(()))?;
                self.accept_edit(
                    &receipt,
                    "Brush",
                    empty.then_some("sampled-first"),
                    self.document().canvas,
                );
                Ok(())
            })();
            if let Err(error) = result {
                self.status = format!("Brush not prepared: {error}");
            }
        }
    }
}
