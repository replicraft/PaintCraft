//! Canvas clearing replaces sparse layers atomically, without visiting pixels.
use super::*;

impl Stage<'_> {
    pub(super) fn clear_canvas(&mut self, index: usize, alias: &str, color: Rgba) -> Result<()> {
        if self.aliases.contains_key(alias) {
            return Err(Error::new(ErrorCode::InputSchema, "duplicate alias"));
        }
        let (width, height) = self.doc.canvas;
        let already_clear = self.doc.roots.len() == 1
            && self.doc.layers_in_order().all(|layer| {
                layer.name == "Background"
                    && layer.origin == (0, 0)
                    && layer.visible
                    && layer.opacity == 255
                    && layer.raster.width() == width
                    && layer.raster.height() == height
                    && layer.raster.default_pixel() == color
                    && self.planning.as_ref().map_or_else(
                        || layer.raster.resident_tiles(),
                        |p| p.resident_tiles(layer.id, &layer.raster),
                    ) == 0
            });
        if already_clear {
            self.aliases
                .insert(alias.to_owned(), Some(self.doc.roots[0]));
        } else {
            let id = deterministic_id(&[
                "layer",
                &self.base_state,
                self.digest,
                &index.to_string(),
                "0",
            ]);
            if self.doc.layers.contains_key(&id) {
                return Err(Error::new(
                    ErrorCode::InvariantViolation,
                    "deterministic identifier collision",
                ));
            }
            if let Some(planning) = &mut self.planning {
                planning.clear();
            }
            self.doc.layers.clear();
            self.doc.roots.clear();
            self.doc.layers.insert(
                id,
                Layer {
                    id,
                    name: "Background".to_owned(),
                    origin: (0, 0),
                    visible: true,
                    opacity: 255,
                    raster: RasterLayer::new(width, height, color),
                },
            );
            self.doc.roots.push(id);
            for target in self.aliases.values_mut() {
                *target = None;
            }
            self.aliases.insert(alias.to_owned(), Some(id));
        }
        self.selection = Selection::Inactive;
        Ok(())
    }
}
