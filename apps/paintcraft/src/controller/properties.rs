//! Native combined properties use one existing activation and atomic batch.
use super::*;
use paintcraft_core::types::{Error, ErrorCode, LayerBlendMode};
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct PropertiesCapture {
    owner: Uuid,
    state: Uuid,
    epoch: u64,
    layer: Uuid,
}
pub(crate) struct PropertiesRequest {
    pub capture: PropertiesCapture,
    pub desired_mode: LayerBlendMode,
    pub values: paintcraft_studio::LayerProperties,
}
#[derive(Debug)]
pub(super) struct QueuedProperties {
    capture: PropertiesCapture,
    desired_mode: LayerBlendMode,
}
fn stale() -> Error {
    Error::new(
        ErrorCode::ConflictStaleBase,
        "Layer properties source or private input changed",
    )
}
impl Editor {
    pub(crate) fn properties_owner(&self) -> Uuid {
        self.session.owner_id()
    }
    pub(crate) fn properties_capture_current(&self, capture: &PropertiesCapture) -> bool {
        let ticket = self.session.ticket();
        capture.owner == self.session.owner_id()
            && capture.state == ticket.state_id
            && capture.epoch == ticket.publication_epoch
            && self.target == Some(capture.layer)
            && self.document().layer(&capture.layer).is_some()
    }
    pub(crate) fn capture_properties(&self, layer: Uuid) -> Result<PropertiesCapture> {
        let ticket = self.session.ticket();
        let capture = PropertiesCapture {
            owner: self.session.owner_id(),
            state: ticket.state_id,
            epoch: ticket.publication_epoch,
            layer,
        };
        if !self.properties_capture_current(&capture)
            || self.properties_input_blocked()
            || self.has_pending_edits()
        {
            return Err(stale());
        }
        Ok(capture)
    }
    fn properties_input_blocked(&self) -> bool {
        self.filter_edit_open
            || self.has_image_paste()
            || self.has_floating_selection()
            || self.gesture_in_progress()
            || self.is_editing_text()
            || self.is_constructing_shape()
            || self.failed_text_retained()
            || self.at_file_barrier()
    }
    pub(crate) fn request_layer_properties(
        &mut self,
        token: Token,
        request: PropertiesRequest,
    ) -> bool {
        let action = Action::SetLayerProperties {
            layer: request.capture.layer,
            name: request.values.name,
            visible: request.values.visible,
            opacity: request.values.opacity,
        };
        self.request_with_properties(
            token,
            action,
            Some(QueuedProperties {
                capture: request.capture,
                desired_mode: request.desired_mode,
            }),
        )
    }
    pub(super) fn validate_properties(
        &self,
        action: &Action,
        edit: &QueuedProperties,
        accepting: bool,
    ) -> Result<()> {
        let Action::SetLayerProperties { layer, name, .. } = action else {
            return Err(Error::new(
                ErrorCode::InvariantViolation,
                "Native properties payload has the wrong action",
            ));
        };
        if name.chars().count() > 1024 {
            return Err(Error::new(
                ErrorCode::InputSchema,
                "Layer name exceeds 1024 characters",
            ));
        }
        if *layer != edit.capture.layer
            || !self.properties_capture_current(&edit.capture)
            || self.properties_input_blocked()
            || (accepting && self.has_pending_edits())
        {
            return Err(stale());
        }
        Ok(())
    }
    pub(super) fn execute_properties(&mut self, action: Action, edit: QueuedProperties) {
        let result = self
            .validate_properties(&action, &edit, false)
            .and_then(|()| {
                let Action::SetLayerProperties {
                    layer,
                    name,
                    visible,
                    opacity,
                } = action
                else {
                    unreachable!()
                };
                let properties = Value::object([
                    ("op", Value::str("layer.set_properties")),
                    ("layer_ref", layer_ref(layer)),
                    ("name", Value::str(name)),
                    ("visible", Value::Bool(visible)),
                    ("opacity_u8", Value::Int(i64::from(opacity))),
                ]);
                let mode = match edit.desired_mode {
                    LayerBlendMode::Normal => "normal",
                    LayerBlendMode::Multiply => "multiply",
                    LayerBlendMode::Screen => "screen",
                };
                let blend = Value::object([
                    ("op", Value::str("layer.set_blend_mode")),
                    ("layer_ref", layer_ref(layer)),
                    ("blend_mode", Value::str(mode)),
                ]);
                strokes::captured_batch(self.session.ticket(), vec![properties, blend])
            });
        match result {
            Ok(batch) => {
                self.commit_batch(batch, "Layer properties changed", None);
            }
            Err(error) => self.status = format!("Edit failed: {error}"),
        }
    }
}
#[cfg(test)]
mod tests;
