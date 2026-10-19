//! Top-left canvas resizing keeps off-canvas data and extends editable layer bounds.
use super::*;

impl Editor {
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
