//! CPU fallback owns derived caches; authoritative documents stay immutable.
use paintcraft_core::types::Result;
use paintcraft_document::Document;
use paintcraft_render::{
    cache::{CacheLimits, CompositeCache, Domain},
    render_cancellable,
};
use std::sync::{Arc, Mutex};

// Measured candidates and the selection are recorded in docs/REGIONAL_COMPOSITOR.md.
// Persistent cache plus simultaneous full/prefix key construction allowances.
pub(super) const ADMISSION_BYTES: u64 = (20 + 2 * 16) << 20;

#[derive(Clone)]
pub(super) struct CpuRenderer(Arc<Mutex<Option<CompositeCache>>>);
impl CpuRenderer {
    pub fn new() -> Self {
        Self(Arc::new(Mutex::new(
            CompositeCache::new(CacheLimits {
                tile_edge: 256,
                committed_bytes: 16 << 20,
                preview_bytes: 4 << 20,
                entries_per_domain: 512,
            })
            .ok(),
        )))
    }

    pub fn render(
        &self,
        doc: &Document,
        preview: bool,
        check: impl Fn() -> Result<()>,
    ) -> Result<Vec<u8>> {
        check()?;
        // Only the derived cache is locked. Native execution has one admitted
        // render worker; the UI never takes this lock while that worker is active.
        let mut guard = self.0.lock().unwrap_or_else(|poisoned| {
            // A worker panic may leave derived entries inconsistent. Retire all
            // of them and keep ordinary CPU rendering available on later frames.
            let mut guard = poisoned.into_inner();
            *guard = None;
            self.0.clear_poison();
            guard
        });
        check()?;
        let Some(cache) = guard.as_mut() else {
            return render_cancellable(doc, check);
        };
        if !preview {
            cache.clear_preview();
        }
        // A sparse single layer was faster without caching in the qualified replay.
        // Tiny outputs also avoid the cache's bounded table/key overhead.
        if doc
            .layers_in_order()
            .filter(|l| {
                l.visible && l.opacity != 0 && doc.canvas_rect().intersect(&l.extent()).is_some()
            })
            .take(2)
            .count()
            < 2
            || doc.canvas_rect().area() < 65536
        {
            return render_cancellable(doc, check);
        }
        cache
            .render(
                doc,
                doc.canvas_rect(),
                if preview {
                    Domain::Preview
                } else {
                    Domain::Committed
                },
                check,
            )
            .map(|out| out.pixels)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{Action, App, Tool, egui};
    use paintcraft_render::render_flat;

    #[test]
    fn desktop_cpu_preview_cancel_commit_and_undo_preserve_exact_frames() {
        let mut app = App::new(None);
        let ctx = egui::Context::default();
        app.editor.request(9101, Action::AddLayer);
        app.editor.run_pending();
        app.sync_texture(&ctx);
        let original = app.rendered.clone();
        let usage = app
            .cpu_renderer
            .0
            .lock()
            .unwrap()
            .as_ref()
            .unwrap()
            .usage(Domain::Committed);
        assert!(usage.pixel_bytes > 0);
        app.editor.select_tool(Tool::Brush);
        app.editor.pointer_down((37, 40));
        app.editor.pointer_move((47, 44));
        app.sync_texture(&ctx);
        let staged = paintcraft_editor::execute::execute(
            app.editor.document(),
            app.editor.selection(),
            &app.editor.preview_batch().unwrap().unwrap(),
        )
        .unwrap();
        assert_eq!(app.rendered, render_flat(&staged.doc).unwrap());
        assert_eq!(
            app.cpu_renderer
                .0
                .lock()
                .unwrap()
                .as_ref()
                .unwrap()
                .usage(Domain::Committed),
            usage
        );
        app.editor.cancel_gesture();
        app.sync_texture(&ctx);
        assert_eq!(app.rendered, original);
        assert_eq!(
            app.cpu_renderer
                .0
                .lock()
                .unwrap()
                .as_ref()
                .unwrap()
                .usage(Domain::Preview)
                .pixel_bytes,
            0
        );
        app.editor.pointer_down((37, 40));
        app.editor.pointer_move((47, 44));
        app.editor.pointer_up();
        app.editor.run_pending();
        app.sync_texture(&ctx);
        let committed = app.rendered.clone();
        assert_ne!(committed, original);
        assert_eq!(committed, render_flat(app.editor.document()).unwrap());
        app.editor.request(9102, Action::Undo);
        app.editor.run_pending();
        app.sync_texture(&ctx);
        assert_eq!(app.rendered, original);
        app.editor.request(9103, Action::Redo);
        app.editor.run_pending();
        app.sync_texture(&ctx);
        assert_eq!(app.rendered, committed);
    }
}

#[cfg(test)]
mod recovery_tests {
    use super::*;
    #[test]
    fn poisoned_derived_cache_is_retired_without_poisoning_future_cpu_frames() {
        let renderer = CpuRenderer::new();
        let failed = renderer.clone();
        assert!(
            std::thread::spawn(move || {
                let _guard = failed.0.lock().unwrap();
                panic!("injected derived-cache worker failure");
            })
            .join()
            .is_err()
        );
        let doc = paintcraft_document::new_document(
            17,
            19,
            Some(paintcraft_core::types::Rgba([12, 34, 56, 255])),
            0,
            paintcraft_document::POLICY,
        )
        .unwrap();
        for _ in 0..2 {
            assert_eq!(
                renderer.render(&doc, false, || Ok(())).unwrap(),
                [12, 34, 56, 255].repeat(17 * 19)
            );
        }
        assert!(renderer.0.lock().unwrap().is_none());
    }
}
