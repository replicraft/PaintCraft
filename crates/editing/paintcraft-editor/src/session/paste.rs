//! External pixels enter through the same private history/publication boundary.
use super::*;
mod onto;
use crate::clipboard::PasteDraft;
use paintcraft_core::types::{Rgba, policy::hex};
use paintcraft_document::{LAYER_COUNT_MAX, Layer};
use paintcraft_history::Label;
use paintcraft_raster::RasterLayer;
use sha2::{Digest, Sha256};

impl Session {
    pub fn paste_image(&mut self, draft: &PasteDraft, name: &str) -> Result<Receipt> {
        self.paste_with_admission(draft, name, Admission::new(self.staged_pixels), &|| Ok(()))
    }
    fn paste_with_admission(
        &mut self,
        draft: &PasteDraft,
        name: &str,
        admission: Admission,
        check: &dyn Fn() -> Result<()>,
    ) -> Result<Receipt> {
        check()?;
        if self.history_ticket() != draft.source {
            return Err(Error::new(
                ErrorCode::ConflictStaleBase,
                "Paste source document or selection changed",
            ));
        }
        if self.doc.layers.len() >= LAYER_COUNT_MAX {
            return Err(Error::new(
                ErrorCode::ResourceLimit,
                "layer count budget exceeded",
            ));
        }
        if name.chars().take(1025).count() > 1024 {
            return Err(Error::new(
                ErrorCode::InputSchema,
                "name exceeds 1024 characters",
            ));
        }
        let mut fingerprint = Sha256::new();
        fingerprint.update(b"paintcraft.paste-rgba/v1");
        fingerprint.update(draft.image.width().to_le_bytes());
        fingerprint.update(draft.image.height().to_le_bytes());
        for bytes in draft.image.rgba().chunks(65536) {
            check()?;
            fingerprint.update(bytes);
        }
        let params = Value::object([
            ("operation", Value::str("paintcraft.paste-layer/v1")),
            ("rgba_sha256", Value::str(hex(&fingerprint.finalize()))),
            ("name", Value::str(name)),
            ("x", Value::Int(draft.origin.0)),
            ("y", Value::Int(draft.origin.1)),
            ("canvas_width", Value::Int(i64::from(draft.canvas_size().0))),
            (
                "canvas_height",
                Value::Int(i64::from(draft.canvas_size().1)),
            ),
        ]);
        let digest = hex(&Sha256::digest(paintcraft_core::types::json::canonical(
            &params,
        )));
        let id = deterministic_id(&["layer", &self.doc.state_id.to_string(), &digest, "paste"]);
        if self.doc.layers.contains_key(&id) {
            return Err(Error::new(
                ErrorCode::InvariantViolation,
                "deterministic identifier collision",
            ));
        }
        let mut admission = admission.constrain_pixels(self.staged_pixels);
        let raster = RasterLayer::from_rgba_checked(
            draft.image.width(),
            draft.image.height(),
            Rgba::TRANSPARENT,
            draft.image.rgba(),
            &mut admission,
            check,
        )?;
        check()?;
        let mut doc = (*self.doc).clone();
        doc.roots
            .try_reserve(1)
            .map_err(|_| Error::new(ErrorCode::ResourceLimit, "cannot grow layer roots"))?;
        doc.layers
            .try_reserve(1)
            .map_err(|_| Error::new(ErrorCode::ResourceLimit, "cannot grow layer index"))?;
        doc.canvas = draft.canvas_size();
        let selection = draft
            .bounds()
            .intersect(&doc.canvas_rect())
            .map_or(Selection::Empty, Selection::Rect);
        doc.roots.push(id);
        doc.layers.insert(
            id,
            Layer {
                id,
                name: name.into(),
                origin: draft.origin,
                visible: true,
                opacity: 255,
                raster,
            },
        );
        self.commit_staged(
            crate::execute::Staged {
                doc,
                selection,
                aliases: BTreeMap::from([("pasted".into(), id)]),
            },
            &digest,
            Label::Paste,
            check,
        )
    }
}
impl Preparation {
    /// Prepare in isolation; the owner's normal publication guard remains required.
    pub fn paste_image(
        mut self,
        draft: &PasteDraft,
        name: &str,
        admission: Admission,
        check: impl Fn() -> Result<()>,
    ) -> Result<PreparedEdit> {
        let receipt = self
            .session
            .paste_with_admission(draft, name, admission, &check)?;
        self.finish_edit(receipt, check)
    }
}
