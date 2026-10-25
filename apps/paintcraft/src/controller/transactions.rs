//! Build typed batches and keep publication-dependent feedback together.
use super::*;

impl Editor {
    pub(super) fn commit(
        &mut self,
        operations: Vec<Value>,
        done: &str,
    ) -> Option<paintcraft_editor::session::Receipt> {
        self.commit_target(operations, done, None)
    }

    pub(super) fn commit_target(
        &mut self,
        operations: Vec<Value>,
        done: &str,
        target_alias: Option<&'static str>,
    ) -> Option<paintcraft_editor::session::Receipt> {
        let t = self.session.ticket();
        let batch = Value::object([
            ("schema", Value::str("paintcraft.batch/v2")),
            ("atomic", Value::Bool(true)),
            ("policy", Value::str(POLICY)),
            (
                "base",
                Value::object([
                    ("state_id", Value::str(t.state_id.to_string())),
                    (
                        "publication_epoch",
                        Value::str(t.publication_epoch.to_string()),
                    ),
                ]),
            ),
            ("operations", Value::Array(operations)),
        ]);
        let batch = match Batch::from_value(&batch) {
            Ok(batch) => batch,
            Err(error) => {
                self.status = format!("Edit failed: {error}");
                return None;
            }
        };
        self.commit_batch(batch, done, target_alias)
    }

    pub(crate) fn commit_batch(
        &mut self,
        batch: Batch,
        done: &str,
        target_alias: Option<&'static str>,
    ) -> Option<paintcraft_editor::session::Receipt> {
        if self.editing.is_some() {
            self.start_edit(batch, done, target_alias);
            return None;
        }
        let before_canvas = self.document().canvas;
        match self.session.apply(&batch) {
            Ok(r) => {
                self.accept_edit(&r, done, target_alias, before_canvas);
                Some(r)
            }
            Err(e) => {
                self.status = format!("Edit failed: {e}");
                None
            }
        }
    }

    pub(super) fn accept_edit(
        &mut self,
        receipt: &paintcraft_editor::session::Receipt,
        done: &str,
        target_alias: Option<&str>,
        before_canvas: (u32, u32),
    ) {
        self.finish_image_paste_publication(target_alias);
        if receipt.committed && self.document().canvas != before_canvas {
            self.canvas_size_edit = Some(self.document().canvas);
        }
        self.status = if receipt.committed {
            done.to_owned()
        } else {
            format!("{done} changed nothing")
        };
        if let Some(alias) = target_alias {
            self.target = receipt.aliases.get(alias).copied().or(self.target);
        }
        self.retarget();
        if receipt.committed && self.session.last_trimmed() > 0 {
            self.status += &format!(
                " - the {} oldest undo step(s) were dropped under the history memory policy",
                self.session.last_trimmed()
            );
        }
    }

    /// Last authored dimension change, excluding history navigation.
    pub(crate) fn take_canvas_size_edit(&mut self) -> Option<(u32, u32)> {
        self.canvas_size_edit.take()
    }

    /// Commit drawing operations; a batch that changes nothing is reported as such.
    pub(super) fn draw(&mut self, ops: Vec<Value>, name: &str) {
        if let Some(r) = self.commit(ops, name)
            && !r.committed
        {
            self.status = format!("{name} changed nothing");
        }
    }
}

impl Editor {
    pub(crate) fn cut_source(
        &self,
        clear: Rgba,
    ) -> Result<paintcraft_editor::clipboard::CutSource> {
        let target = self.target.ok_or_else(|| {
            paintcraft_core::types::Error::new(
                paintcraft_core::types::ErrorCode::ReferenceUnknownId,
                "Cut requires a target layer",
            )
        })?;
        paintcraft_editor::clipboard::CutSource::new(&self.session, target, clear)
    }

    pub(crate) fn complete_cut(
        &mut self,
        source: &paintcraft_editor::clipboard::CutSource,
    ) -> Result<()> {
        let batch = source.removal_batch(&self.session, self.target)?;
        self.commit_batch(batch, "Cut", None);
        Ok(())
    }
}

impl Editor {
    pub(crate) fn paste_draft(
        &self,
        image: paintcraft_core::types::clipboard::Image,
    ) -> Result<paintcraft_editor::clipboard::PasteDraft> {
        paintcraft_editor::clipboard::PasteDraft::new(&self.session, image)
    }
    pub(crate) fn start_paste(
        &mut self,
        draft: paintcraft_editor::clipboard::PasteDraft,
        name: String,
    ) -> Result<()> {
        self.start_paste_destination(draft, super::retry::PasteDestination::NewLayer(name))
    }

    /// Publish a frozen clipboard placement onto the captured active layer.
    /// The UI must finish private positioning before invoking this transaction.
    pub fn start_paste_onto(
        &mut self,
        draft: paintcraft_editor::clipboard::PasteDraft,
        target: Uuid,
        blend: paintcraft_core::types::policy::BlendMode,
        key: Option<Rgba>,
    ) -> Result<()> {
        if self.target != Some(target) {
            return Err(paintcraft_core::types::Error::new(
                paintcraft_core::types::ErrorCode::ConflictStaleBase,
                "Paste target changed",
            ));
        }
        self.start_paste_destination(
            draft,
            super::retry::PasteDestination::ExistingLayer { target, blend, key },
        )
    }

    fn start_paste_destination(
        &mut self,
        draft: paintcraft_editor::clipboard::PasteDraft,
        destination: super::retry::PasteDestination,
    ) -> Result<()> {
        if self.has_pending_edits() || self.file_barrier.is_some() || self.has_image_paste() {
            return Err(paintcraft_core::types::Error::new(
                paintcraft_core::types::ErrorCode::ConflictStaleBase,
                "Finish pending edits before pasting",
            ));
        }
        if draft.source() != self.session.history_ticket() {
            return Err(paintcraft_core::types::Error::new(
                paintcraft_core::types::ErrorCode::ConflictStaleBase,
                "Paste source changed",
            ));
        }
        let before_canvas = self.document().canvas;
        if let Some(service) = &self.editing {
            let admission = service.admission(u64::MAX);
            let task = self.session.preparation().map(|preparation| {
                Box::new(move |cancellation: paintcraft_runtime::Cancellation| {
                    super::retry::prepare_paste(
                        preparation,
                        Arc::new(draft),
                        destination,
                        admission,
                        || cancellation.check(),
                    )
                }) as super::dispatch::Task
            });
            self.dispatch_initial(task, "Paste".into(), "", Some("pasted"));
        } else {
            let receipt = match destination {
                super::retry::PasteDestination::NewLayer(name) => {
                    self.session.paste_image(&draft, &name)?
                }
                super::retry::PasteDestination::ExistingLayer { target, blend, key } => {
                    self.session.paste_image_onto(&draft, target, blend, key)?
                }
            };
            self.accept_edit(&receipt, "Paste", Some("pasted"), before_canvas);
        }
        Ok(())
    }
}
