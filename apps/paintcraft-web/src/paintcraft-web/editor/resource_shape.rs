//! Synchronous browser drag conversion refuses the entire request above its bound.
use super::*;
use paintcraft_core::types::resources::SharedBudget;
use paintcraft_formats::csh::{
    self,
    placement::{self, Policy},
};
use paintcraft_tools::{
    Tool,
    resource_shape::{Placement, Reference},
};
use std::sync::Arc;
pub(crate) const DRAG_PIXELS_MAX: u64 = 262_144;
#[derive(Debug)]
pub(super) struct Captured {
    reference: Reference,
    library: Arc<csh::Library>,
    name: String,
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
        self.cancel();
        self.resource_shape = Some(Captured {
            reference,
            library,
            name,
        });
        self.tools.select_resource_shape(reference);
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
    pub(super) fn resource_shape_operations(&self, request: Placement) -> Result<Vec<Value>> {
        let captured = self
            .resource_shape
            .as_ref()
            .filter(|s| {
                s.reference == request.resource && s.reference.policy == Policy::CubicFillV1.id()
            })
            .ok_or_else(|| {
                Error::new(
                    ErrorCode::ConflictStaleBase,
                    "The selected custom shape is no longer available",
                )
            })?;
        let doc = self.session.document();
        let empty = doc.layers.is_empty() && doc.roots.is_empty();
        if !(empty && request.layer.is_nil())
            && !doc
                .layers
                .get(&request.layer)
                .is_some_and(|layer| layer.visible)
        {
            return Err(Error::new(
                ErrorCode::TargetInvalid,
                "Choose a visible raster layer before placing a shape",
            ));
        }
        let (at, width, height) = request.target_box(placement::SIDE_MAX, DRAG_PIXELS_MAX)?;
        let budget = SharedBudget::new(32 << 20);
        let raster = placement::rasterize(
            captured.library.clone(),
            captured.reference.preset,
            width,
            height,
            Policy::CubicFillV1,
            &budget,
            &|| Ok(()),
        )?;
        let coverage = raster.translated_spans(at, doc.canvas_rect(), &budget, &|| Ok(()))?;
        if empty {
            commands::first_layer_mask_operations(
                doc.canvas,
                request.color,
                coverage.spans(),
                "shape-first",
                "Shape",
                &|| Ok(()),
            )
        } else {
            commands::mask_operations(request.layer, request.color, coverage.spans(), &|| Ok(()))
        }
    }
    pub(super) fn commit_resource_shape(&mut self, request: Placement) -> Result<()> {
        let operations = self.resource_shape_operations(request)?;
        if operations.is_empty() {
            return Ok(());
        }
        let batch = self.batch(operations)?;
        let empty =
            self.session.document().layers.is_empty() && self.session.document().roots.is_empty();
        let preparation = self.session.preparation()?;
        let prepared = if empty {
            preparation.apply_initial_shape_mask(&batch, || Ok(()))?
        } else {
            preparation.apply_shape_mask(&batch, || Ok(()))?
        };
        let receipt = self.session.publish_prepared(prepared, || Ok(()))?;
        if receipt.committed {
            self.target = receipt.aliases.get("shape-first").copied().or(self.target);
            self.retarget();
        }
        Ok(())
    }
}
#[cfg(test)]
mod tests;
