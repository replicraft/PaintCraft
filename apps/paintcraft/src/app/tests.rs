use super::*;

#[cfg(test)]
mod save_binding_tests {
    use super::*;

    #[test]
    fn failed_save_as_keeps_the_previous_document_binding() {
        let dir = std::env::temp_dir().join(format!("paintcraft-save-binding-{}", Uuid::new_v4()));
        let old = dir.join("original.pcnd");
        let mut app = App::new(None);
        app.bound = Some(old.clone());
        app.editor.set_path(old.clone());
        app.editor.request(123, Action::AddLayer);
        app.editor.run_pending();
        let destination = dir.join("missing/new.pcnd");
        app.save_selected(&egui::Context::default(), destination.clone());
        app.editor.run_pending();
        app.finish_save();
        assert_eq!(app.bound.as_deref(), Some(old.as_path()));
        assert_eq!(app.editor.path(), Some(old.as_path()));
        assert!(app.editor.is_dirty());
        assert!(!destination.exists());
    }

    #[test]
    fn successful_save_as_binds_only_after_the_file_is_confirmed() {
        let dir = std::env::temp_dir().join(format!("paintcraft-save-binding-{}", Uuid::new_v4()));
        std::fs::create_dir(&dir).unwrap();
        let path = dir.join("saved.pcnd");
        let mut app = App::new(None);
        app.save_selected(&egui::Context::default(), path.clone());
        assert!(app.bound.is_none());
        app.editor.run_pending();
        app.finish_save();
        assert_eq!(app.bound.as_deref(), Some(path.as_path()));
        assert!(!app.editor.is_dirty());
        assert!(
            Editor::open(&path)
                .unwrap()
                .document()
                .same_semantics(app.editor.document())
        );
        std::fs::remove_dir_all(dir).unwrap();
    }
}

#[cfg(test)]
mod render_worker_tests {
    use super::*;
    use std::{sync::mpsc, time::Instant};

    #[test]
    fn cancelled_preview_cannot_publish_over_a_new_gesture_on_the_same_state() {
        let mut app = App::new(None);
        let ctx = egui::Context::default();
        app.editor.select_tool(Tool::Brush);
        app.editor.pointer_down((8, 8));
        app.sync_texture(&ctx);
        let old = app.presentation_key();
        let (release, wait) = mpsc::sync_channel(1);
        let (started, wait_started) = mpsc::sync_channel(1);
        let mut executor = Executor::new(WorkerLimits {
            workers: 1,
            pending_jobs: 2,
            transient_bytes: 512 << 20,
        })
        .unwrap();
        let handle = executor
            .submit(
                1024,
                move |_| {
                    started.send(()).unwrap();
                    wait.recv().unwrap();
                    Ok(RenderedFrame {
                        key: old,
                        pixels: vec![0; 320 * 320 * 4],
                        fallback: None,
                    })
                },
                || {},
            )
            .unwrap();
        wait_started.recv_timeout(Duration::from_secs(5)).unwrap();
        app.renderer = Some(executor);
        app.pending_frame = Some((old, handle));
        app.editor.cancel_gesture();
        app.editor.pointer_down((30, 40));
        app.sync_texture(&ctx);
        let current = app.presentation_key();
        assert_eq!((old.0, old.1), (current.0, current.1));
        assert_ne!(old.2, current.2);
        release.send(()).unwrap();
        let deadline = Instant::now() + Duration::from_secs(5);
        while app.texture_key != Some(current) {
            assert!(Instant::now() < deadline);
            app.sync_texture(&ctx);
            std::thread::yield_now();
        }
        let doc = paintcraft_editor::execute::execute(
            app.editor.document(),
            app.editor.selection(),
            &app.editor.preview_batch().unwrap().unwrap(),
        )
        .unwrap()
        .doc;
        assert_eq!(app.rendered, render_flat(&doc).unwrap());
        assert_eq!(app.renderer.as_ref().unwrap().reserved_bytes(), 0);
    }

    #[test]
    fn an_old_completion_cannot_publish_after_a_new_edit() {
        let mut app = App::new(None);
        let ctx = egui::Context::default();
        let mut executor = Executor::new(WorkerLimits {
            workers: 1,
            pending_jobs: 2,
            transient_bytes: 8 << 20,
        })
        .unwrap();
        let old = app.presentation_key();
        let (release, wait) = mpsc::sync_channel(1);
        let (started, wait_started) = mpsc::sync_channel(1);
        let handle = executor
            .submit(
                1024,
                move |_| {
                    started.send(()).unwrap();
                    wait.recv().unwrap();
                    Ok(RenderedFrame {
                        key: old,
                        pixels: vec![0; 320 * 320 * 4],
                        fallback: None,
                    })
                },
                || {},
            )
            .unwrap();
        wait_started.recv_timeout(Duration::from_secs(5)).unwrap();
        app.renderer = Some(executor);
        app.pending_frame = Some((old, handle));
        app.editor.request(12345, Action::AddLayer);
        app.editor.run_pending();
        let current = app.presentation_key();
        assert_ne!(old, current);
        app.sync_texture(&ctx);
        release.send(()).unwrap();
        let deadline = Instant::now() + Duration::from_secs(5);
        while app.texture_key != Some(current) {
            assert!(Instant::now() < deadline, "current render did not finish");
            app.sync_texture(&ctx);
            std::thread::yield_now();
        }
        assert_eq!(app.rendered, render_flat(app.editor.document()).unwrap());
        assert!(
            app.rendered
                .as_chunks::<4>()
                .0
                .iter()
                .all(|pixel| pixel == &[255; 4])
        );
        assert_eq!(app.renderer.as_ref().unwrap().reserved_bytes(), 0);
    }
}

#[cfg(test)]
mod recovery_handoff_tests {
    use super::*;
    fn orphan(dir: &Path) -> Found {
        let recovery = Recovery::start(dir).unwrap();
        let doc = paintcraft_document::new_document(
            12,
            12,
            Some(Rgba([17, 22, 33, 255])),
            47,
            paintcraft_document::POLICY,
        )
        .unwrap();
        recovery.checkpoint_document(&doc, None).unwrap();
        drop(recovery);
        Recovery::scan(dir).into_iter().next().unwrap()
    }
    #[test]
    fn restored_work_is_checkpointed_before_the_old_orphan_is_discarded() {
        let dir =
            std::env::temp_dir().join(format!("paintcraft-recovery-handoff-{}", Uuid::new_v4()));
        let old = orphan(&dir);
        let mut app = App::new(None).with_recovery(dir.clone());
        app.recover_found(&old).unwrap();
        assert!(!old.checkpoint.exists());
        assert!(app.editor.is_dirty());
        assert!(app.editor.has_checkpoint());
        let expected = app.editor.document_snapshot();
        drop(app); // crash immediately after handoff, before the periodic timer
        let found = Recovery::scan(&dir);
        assert_eq!(found.len(), 1);
        assert!(
            Recovery::read_checkpoint(&found[0].checkpoint)
                .unwrap()
                .same_semantics(&expected)
        );
        std::fs::remove_dir_all(dir).unwrap();
    }
    #[test]
    fn failed_new_recovery_store_preserves_the_original_orphan() {
        let dir = std::env::temp_dir().join(format!(
            "paintcraft-recovery-handoff-failed-{}",
            Uuid::new_v4()
        ));
        let old = orphan(&dir);
        let mut app = App::new(None).with_recovery(dir.clone());
        let blocked = dir.join("not-a-directory");
        std::fs::write(&blocked, b"block new recovery creation").unwrap();
        app.recovery_dir = Some(blocked);
        app.recover_found(&old).unwrap();
        assert!(app.editor.is_dirty());
        assert!(old.checkpoint.exists());
        assert!(
            app.editor
                .status()
                .contains("original recovery copy is retained")
        );
        drop(app);
        assert_eq!(Recovery::scan(&dir).len(), 1);
        std::fs::remove_dir_all(dir).unwrap();
    }
}
