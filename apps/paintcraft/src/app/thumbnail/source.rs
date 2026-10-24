//! Immutable overview inputs; main canvas and overview share preview preparation.
use super::*;
use paintcraft_document::Document;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) struct Scope {
    document: Uuid,
    state: Uuid,
    epoch: u64,
    preview: Option<controller::PreviewStamp>,
    owner: Option<(Uuid, Option<u64>)>,
}

impl Scope {
    #[cfg(test)]
    pub fn committed(doc: &Document) -> Self {
        Self {
            document: doc.document_id,
            state: doc.state_id,
            epoch: 0,
            preview: None,
            owner: None,
        }
    }

    /// An earlier frame may be displayed as updating within the same gesture,
    /// or as the committed baseline for that gesture. It never becomes current.
    pub fn can_progress_to(self, next: Self) -> bool {
        self.document == next.document
            && self.state == next.state
            && self.epoch == next.epoch
            && (self.owner.is_none() || (self.owner == next.owner && next.owner.is_some()))
    }
}

pub(super) struct Source {
    pub doc: Arc<Document>,
    pub scope: Scope,
    preparation: Option<controller::PreviewPreparation>,
    selection: Selection,
}

impl Source {
    #[cfg(test)]
    pub fn committed(doc: Arc<Document>) -> Self {
        Self {
            scope: Scope::committed(&doc),
            doc,
            preparation: None,
            selection: Selection::Inactive,
        }
    }

    pub fn capture(editor: &controller::Editor) -> Self {
        let doc = editor.document_snapshot();
        let preview = editor
            .preview_snapshot()
            .filter(|preview| !matches!(preview.as_ref(), Preview::Marquee { .. }));
        // Read after the main canvas has retained its snapshot: no second path
        // clone or command construction is needed on the UI thread.
        let scope = Scope {
            document: doc.document_id,
            state: doc.state_id,
            epoch: editor.publications(),
            preview: preview.as_ref().map(|_| editor.preview_stamp()),
            owner: preview.as_ref().map(|_| editor.preview_owner()),
        };
        Self {
            doc,
            scope,
            preparation: preview.map(|preview| editor.capture_preview(Some(preview))),
            selection: editor.selection(),
        }
    }

    pub fn has_preview(&self) -> bool {
        self.preparation
            .as_ref()
            .is_some_and(|preparation| preparation.possible_preview())
    }

    pub fn render(
        self,
        check: impl Fn() -> paintcraft_core::types::Result<()>,
    ) -> paintcraft_core::types::Result<DisplayFrame> {
        check()?;
        let staged;
        let doc = match self
            .preparation
            .map(|preparation| preparation.build(&check))
            .transpose()?
            .flatten()
        {
            Some(batch) => {
                staged = paintcraft_editor::execute::execute_with_admission_checked(
                    &self.doc,
                    self.selection,
                    &batch,
                    paintcraft_raster::Admission::new(
                        paintcraft_core::types::policy::STAGED_PIXELS_MAX,
                    ),
                    &check,
                )?;
                &staged.doc
            }
            None => self.doc.as_ref(),
        };
        check()?;
        paintcraft_render::thumbnail::render_document(doc, SIDE, check)
    }
}

#[cfg(test)]
#[path = "source_tests.rs"]
mod tests;
