//! Top-left canvas resizing keeps off-canvas data and extends editable layer bounds.
use super::*;

#[cfg(test)]
#[path = "canvas/tests.rs"]
mod tests;

impl Editor {
    pub(super) fn transform_image(
        &mut self,
        transform: paintcraft_core::types::orthogonal::OrthogonalTransform,
        clear: Rgba,
    ) {
        match paintcraft_tools::commands::transform_image(
            self.selection(),
            self.target,
            transform,
            clear,
        ) {
            Ok(op) => {
                self.cancel_gesture();
                self.draw(vec![op], "Transform image");
            }
            Err(e) => self.status = e.to_string(),
        }
    }

    pub(super) fn crop_to_selection(&mut self) {
        match paintcraft_tools::commands::crop_to_selection(self.document(), self.selection()) {
            Ok(op) => {
                self.cancel_gesture();
                self.draw(vec![op], "Crop canvas");
            }
            Err(error) => self.status = error.to_string(),
        }
    }
    /// Solid canvas ground used by Classic; imported images fall back to white.
    pub fn canvas_color(&self) -> Rgba {
        paintcraft_tools::commands::canvas_color(self.document())
    }

    pub(super) fn clear_canvas(&mut self, color: Rgba) {
        self.cancel_gesture();
        let ops = vec![Value::object([
            ("op", Value::str("canvas.clear")),
            ("ref", Value::str("background")),
            ("color", Value::str(color.to_hex())),
        ])];
        if let Some(receipt) = self.commit_target(
            ops,
            "Canvas cleared — Undo restores all layers",
            Some("background"),
        ) {
            self.target = receipt.aliases.get("background").copied().or(self.target);
        }
    }

    pub(super) fn resize_canvas(&mut self, width: u32, height: u32) {
        let ops = paintcraft_tools::commands::resize_canvas(self.document(), width, height);
        if !ops.is_empty() {
            self.draw(ops, "Canvas size");
        }
    }
}
