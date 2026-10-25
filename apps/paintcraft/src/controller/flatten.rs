//! Destructive stack changes use the same ordered worker as ordinary edits.
use super::*;
use paintcraft_runtime::Cancellation;

impl Editor {
    pub(super) fn flatten_image(&mut self) {
        self.cancel_gesture();
        if let Some(service) = &self.editing {
            let admission = service.admission(u64::MAX);
            let task = self.session.preparation().map(|preparation| {
                Box::new(move |cancellation: Cancellation| {
                    preparation
                        .flatten(
                            |doc, check| paintcraft_render::render_cancellable(doc, check),
                            admission,
                            || cancellation.check(),
                        )
                        .map(background::Completion::Edit)
                }) as dispatch::Task
            });
            self.dispatch_initial(task, "Flatten image".into(), "", Some("flattened"));
            return;
        }
        let canvas = self.document().canvas;
        match self
            .session
            .flatten(|doc, check| paintcraft_render::render_cancellable(doc, check))
        {
            Ok(receipt) => self.accept_edit(&receipt, "Flatten image", Some("flattened"), canvas),
            Err(error) => self.status = format!("Edit failed: {error}"),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use eframe::egui;
    use std::time::{Duration, Instant};

    fn setup() -> Editor {
        let mut editor = Editor::new_document(12, 8).unwrap();
        editor.request(1, Action::AddLayer);
        editor.run_pending();
        editor.request(
            2,
            Action::Fill {
                layer: editor.target().unwrap(),
                at: (1, 1),
                color: Rgba([170, 30, 70, 128]),
                tolerance: 0,
            },
        );
        editor.run_pending();
        editor
    }
    #[test]
    fn sync_and_native_worker_flatten_retarget_and_keep_fifo_undo_redo() {
        for worker in [false, true] {
            let mut editor = setup();
            let original = editor.document_snapshot();
            let original_depth = editor.undo_depth();
            let pixels = paintcraft_render::render_flat(&original).unwrap();
            if worker {
                editor.connect_editing(background::Service::new(egui::Context::default()).unwrap());
            }
            editor.request(3, Action::FlattenImage);
            let deadline = Instant::now() + Duration::from_secs(10);
            while editor.has_pending_edits() {
                editor.run_pending();
                assert!(Instant::now() < deadline);
                std::thread::yield_now();
            }
            assert_eq!(editor.document().roots.len(), 1);
            assert_eq!(editor.target(), editor.document().roots.last().copied());
            assert_eq!(editor.undo_depth(), original_depth + 1);
            assert_eq!(
                paintcraft_render::render_flat(editor.document()).unwrap(),
                pixels
            );
            let flattened = editor.document_snapshot();
            editor.request(4, Action::Undo);
            while editor.has_pending_edits() {
                editor.run_pending();
                assert!(Instant::now() < deadline);
                std::thread::yield_now();
            }
            assert!(editor.document().same_semantics(&original));
            editor.request(5, Action::Redo);
            while editor.has_pending_edits() {
                editor.run_pending();
                assert!(Instant::now() < deadline);
                std::thread::yield_now();
            }
            assert!(editor.document().same_semantics(&flattened));
        }
    }
    #[test]
    fn flatten_waits_for_earlier_edits_and_later_undo_uses_its_recorded_source() {
        let mut editor = setup();
        let service = background::Service::new(egui::Context::default()).unwrap();
        let release = service.pause();
        editor.connect_editing(service);
        let source = editor.document_snapshot();
        let depth = editor.undo_depth();
        editor.request(10, Action::AddLayer);
        editor.request(11, Action::FlattenImage);
        editor.request(12, Action::Undo);
        assert_eq!(editor.run_pending(), 0);
        assert!(editor.document().same_semantics(&source));
        release.send(()).unwrap();
        let deadline = Instant::now() + Duration::from_secs(10);
        while editor.has_pending_edits() {
            editor.run_pending();
            assert!(Instant::now() < deadline);
            std::thread::yield_now();
        }
        assert_eq!(editor.document().roots.len(), source.roots.len() + 1);
        assert_eq!(editor.undo_depth(), depth + 1);
        assert_eq!(editor.redo_depth(), 1);
    }
}
