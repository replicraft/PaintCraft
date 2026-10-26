//! Resource gestures resolve their captured immutable library on existing workers.
use super::*;
use paintcraft_core::types::{Error, ErrorCode, Rect, resources::SharedBudget};
use paintcraft_formats::csh::{
    self,
    placement::{self, Policy},
};
use paintcraft_tools::resource_shape::{Placement, Reference};

#[derive(Clone, Debug)]
pub(super) struct Captured {
    reference: Reference,
    library: Arc<csh::Library>,
    name: String,
}
impl Captured {
    pub(super) fn operations(
        &self,
        request: Placement,
        canvas: Rect,
        empty: bool,
        check: &dyn Fn() -> Result<()>,
    ) -> Result<Vec<Value>> {
        if request.resource != self.reference || request.resource.policy != Policy::CubicFillV1.id()
        {
            return Err(Error::new(
                ErrorCode::ConflictStaleBase,
                "The selected custom shape is no longer available",
            ));
        }
        let (at, width, height) = request.target_box(placement::SIDE_MAX, placement::PIXELS_MAX)?;
        let budget = SharedBudget::new(96 << 20);
        let raster = placement::rasterize(
            self.library.clone(),
            self.reference.preset,
            width,
            height,
            Policy::CubicFillV1,
            &budget,
            check,
        )?;
        let coverage = raster.translated_spans(at, canvas, &budget, check)?;
        if empty {
            paintcraft_tools::commands::first_layer_mask_operations(
                (canvas.width() as u32, canvas.height() as u32),
                request.color,
                coverage.spans(),
                "shape-first",
                "Shape",
                &check,
            )
        } else {
            paintcraft_tools::commands::mask_operations(
                request.layer,
                request.color,
                coverage.spans(),
                &check,
            )
        }
    }
}
impl Editor {
    pub(crate) fn use_resource_shape(
        &mut self,
        reference: Reference,
        library: Arc<csh::Library>,
        name: String,
    ) -> Result<()> {
        self.shape_target()?;
        if reference.policy != Policy::CubicFillV1.id()
            || !library.shapes().get(reference.preset).is_some_and(|shape| {
                placement::assess(shape, &|| Ok(())).is_ok_and(|a| a.qualified())
            })
        {
            return Err(Error::new(
                ErrorCode::CapabilityUnsupported,
                "This shape cannot be placed",
            ));
        }
        self.select_tool(Tool::Rect);
        self.resource_shape = Some(Captured {
            reference,
            library,
            name,
        });
        self.tools.select_resource_shape(reference);
        self.status = "Custom shape tool selected.".into();
        Ok(())
    }
    pub(super) fn resource_shape_owner(&self, reference: Option<Reference>) -> bool {
        reference.is_none_or(|reference| {
            self.resource_shape
                .as_ref()
                .is_some_and(|source| source.reference == reference)
        })
    }
    pub(crate) fn resource_shape_name(&self) -> Option<&str> {
        self.resource_shape
            .as_ref()
            .filter(|shape| {
                self.tools.tool == Tool::Rect && self.tools.resource_shape == Some(shape.reference)
            })
            .map(|shape| shape.name.as_str())
    }
    pub(super) fn resource_shape_stroke(&mut self, shape: Placement) {
        if let Err(error) = shape.target_box(placement::SIDE_MAX, placement::PIXELS_MAX) {
            self.status = error.message;
            return;
        }
        if !self.resource_shape_owner(Some(shape.resource)) {
            self.status = "The selected custom shape is no longer available".into();
            return;
        }
        let plan = self.prepare_preview(Some(Preview::ResourceShape(shape)));
        let empty = self.document().layers.is_empty() && self.document().roots.is_empty();
        if let Some(service) = &self.editing {
            let admission = service.admission(u64::MAX);
            let task = self.session.preparation().map(|preparation| {
                Box::new(move |cancel: paintcraft_runtime::Cancellation| {
                    let check = || cancel.check();
                    let prepared = match plan.build(check)? {
                        None => preparation.no_change(check)?,
                        Some(batch) if empty => preparation
                            .apply_initial_shape_mask_with_admission(&batch, admission, check)?,
                        Some(batch) => {
                            preparation.apply_shape_mask_with_admission(&batch, admission, check)?
                        }
                    };
                    Ok(background::Completion::Edit(prepared))
                }) as dispatch::Task
            });
            self.dispatch_initial(task, "Shape".into(), "", empty.then_some("shape-first"));
        } else {
            let result: Result<()> = (|| {
                let Some(batch) = plan.build(|| Ok(()))? else {
                    return Ok(());
                };
                let preparation = self.session.preparation()?;
                let prepared = if empty {
                    preparation.apply_initial_shape_mask(&batch, || Ok(()))?
                } else {
                    preparation.apply_shape_mask(&batch, || Ok(()))?
                };
                let receipt = self.session.publish_prepared(prepared, || Ok(()))?;
                self.accept_edit(
                    &receipt,
                    "Shape",
                    empty.then_some("shape-first"),
                    self.document().canvas,
                );
                Ok(())
            })();
            if let Err(error) = result {
                self.status = if error.message.starts_with("Custom shape size")
                    || error.message == "The selected custom shape is no longer available"
                {
                    error.message
                } else {
                    format!("Stroke not prepared: {error}")
                };
            }
        }
    }
}
#[cfg(test)]
#[path = "resource_shape/tests.rs"]
mod tests;
