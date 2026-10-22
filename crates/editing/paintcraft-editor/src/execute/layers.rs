//! Structural edits preserve the complete raster, independently of selection.
use super::*;

impl Stage<'_> {
    pub(super) fn duplicate_layer(
        &mut self,
        index: usize,
        source: &LayerRef,
        alias: &str,
        name: &str,
    ) -> Result<()> {
        if self.aliases.contains_key(alias) {
            return Err(Error::new(
                ErrorCode::InputSchema,
                format!("duplicate alias {alias:?}"),
            ));
        }
        let source = self.resolve(source)?;
        if self.doc.layers.len() >= LAYER_COUNT_MAX {
            return Err(Error::new(
                ErrorCode::ResourceLimit,
                "layer count budget exceeded",
            ));
        }
        let position = self
            .doc
            .roots
            .iter()
            .position(|id| *id == source)
            .ok_or_else(|| {
                Error::new(
                    ErrorCode::InvariantViolation,
                    "source is absent from layer stack",
                )
            })?;
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
        // Raster clones share immutable tile storage. Future writes use COW;
        // transparent RGB, default pixels and off-canvas tiles are retained.
        let mut copy = self.doc.layers[&source].clone();
        copy.id = id;
        copy.name = name.to_owned();
        self.doc.layers.insert(id, copy);
        self.doc.roots.insert(position + 1, id);
        self.aliases.insert(alias.to_owned(), Some(id));
        if let Some(planning) = &mut self.planning {
            planning.duplicate(source, id, &self.adm, self.check)?;
        }
        Ok(())
    }
}
