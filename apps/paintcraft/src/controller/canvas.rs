//! Top-left canvas resizing keeps off-canvas data and extends editable layer bounds.
use super::*;

#[cfg(test)]
#[path = "canvas/tests.rs"]
mod tests;

impl Editor {
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
