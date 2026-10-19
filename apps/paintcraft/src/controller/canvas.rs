//! Top-left canvas resizing keeps off-canvas data and extends editable layer bounds.
use super::*;

#[cfg(test)]
#[path = "canvas/tests.rs"]
mod tests;

impl Editor {
    /// Solid canvas ground used by Classic; imported images fall back to white.
    pub fn canvas_color(&self) -> Rgba {
        self.document()
            .layers_in_order()
            .map(|layer| layer.raster.default_pixel())
            .find(|color| color.a() == 255)
            .unwrap_or(Rgba([255; 4]))
    }

    pub(super) fn clear_canvas(&mut self, color: Rgba) {
        self.cancel_gesture();
        let ops = vec![Value::object([
            ("op", Value::str("canvas.clear")),
            ("ref", Value::str("background")),
            ("color", Value::str(color.to_hex())),
        ])];
        if let Some(receipt) = self.commit(ops, "Canvas cleared — Undo restores all layers") {
            self.target = receipt.aliases.get("background").copied().or(self.target);
        }
    }

    pub(super) fn resize_canvas(&mut self, width: u32, height: u32) {
        let mut ops = vec![Value::object([
            ("op", Value::str("canvas.resize")),
            ("width", Value::Int(width.into())),
            ("height", Value::Int(height.into())),
            ("anchor", Value::str("top-left")),
            ("outside_content", Value::str("preserve")),
        ])];
        for layer in self.document().layers_in_order() {
            let w = i64::from(layer.raster.width()).max(i64::from(width) - layer.origin.0);
            let h = i64::from(layer.raster.height()).max(i64::from(height) - layer.origin.1);
            if w > i64::from(layer.raster.width()) || h > i64::from(layer.raster.height()) {
                ops.push(Value::object([
                    ("op", Value::str("layer.expand")),
                    ("layer_ref", layer_ref(layer.id)),
                    ("width", Value::Int(w)),
                    ("height", Value::Int(h)),
                ]));
            }
        }
        self.draw(ops, "Canvas size");
    }
}
