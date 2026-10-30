//! Flattening is a captured-source transaction; rendering is supplied by the host.
//! The editor owns pixel admission, recorded history and atomic publication.
use super::*;
use paintcraft_core::types::{Rgba, policy::hex};
use paintcraft_document::Layer;
use paintcraft_history::Label;
use paintcraft_raster::RasterLayer;
use sha2::{Digest, Sha256};

impl Session {
    /// Flatten visible composition into one canvas-sized layer. Hidden layers and
    /// off-canvas pixels are removed; Undo retains their complete prior state.
    /// The callback receives this transaction's immutable source, never live UI state.
    /// With zero or one layer this is a no-op and the callback is not called.
    pub fn flatten(
        &mut self,
        render: impl FnOnce(&Document, &dyn Fn() -> Result<()>) -> Result<Vec<u8>>,
    ) -> Result<Receipt> {
        self.flatten_checked(render, Admission::new(self.staged_pixels), &|| Ok(()))
    }

    fn flatten_checked(
        &mut self,
        render: impl FnOnce(&Document, &dyn Fn() -> Result<()>) -> Result<Vec<u8>>,
        admission: Admission,
        check: &dyn Fn() -> Result<()>,
    ) -> Result<Receipt> {
        check()?;
        if self.doc.roots.len() <= 1 {
            return Ok(Receipt {
                committed: false,
                state_id: self.doc.state_id,
                publication_epoch: self.epoch,
                history_id: None,
                aliases: BTreeMap::new(),
                filter_aliases: BTreeMap::new(),
                document_features: masked_document_features(&self.doc),
            });
        }
        let (width, height) = self.doc.canvas;
        let rgba = render(&self.doc, check)?;
        check()?;
        let mut admission = admission.constrain_pixels(self.staged_pixels);
        let default = rgba
            .get(..4)
            .and_then(|pixel| pixel.try_into().ok())
            .map(Rgba)
            .unwrap_or(Rgba::TRANSPARENT);
        let raster =
            RasterLayer::from_rgba_checked(width, height, default, &rgba, &mut admission, check)?;
        let mut hash = Sha256::new();
        hash.update(b"paintcraft.flatten/v1");
        hash.update(width.to_le_bytes());
        hash.update(height.to_le_bytes());
        for chunk in rgba.chunks(65536) {
            check()?;
            hash.update(chunk);
        }
        let digest = hex(&hash.finalize());
        drop(rgba);
        let id = deterministic_id(&["layer", &self.doc.state_id.to_string(), &digest, "flatten"]);
        if self.doc.layers.contains_key(&id) {
            return Err(Error::new(
                ErrorCode::InvariantViolation,
                "deterministic identifier collision",
            ));
        }
        let mut doc = (*self.doc).clone();
        doc.layers.clear();
        doc.roots.clear();
        doc.roots.push(id);
        doc.layers.insert(
            id,
            Layer {
                id,
                name: "Background".into(),
                origin: (0, 0),
                visible: true,
                opacity: 255,
                filters: Default::default(),
                raster,
            },
        );
        self.commit_staged(
            crate::execute::Staged {
                doc,
                selection: self.selection.clone(),
                aliases: BTreeMap::from([("flattened".into(), id)]),
                filter_aliases: BTreeMap::new(),
            },
            &digest,
            Label::FlattenImage,
            check,
        )
    }
}

impl Preparation {
    /// Prepare on a worker, then use `Session::publish_prepared` on the owner.
    pub fn flatten(
        mut self,
        render: impl FnOnce(&Document, &dyn Fn() -> Result<()>) -> Result<Vec<u8>>,
        admission: Admission,
        check: impl Fn() -> Result<()>,
    ) -> Result<PreparedEdit> {
        let receipt = self.session.flatten_checked(render, admission, &check)?;
        self.finish_edit(receipt, check)
    }
}
