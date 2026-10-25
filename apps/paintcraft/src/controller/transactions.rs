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
