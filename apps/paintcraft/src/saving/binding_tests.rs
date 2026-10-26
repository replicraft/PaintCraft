use super::tests::{app, drain, scratch};
use super::*;
use std::{fs, sync::mpsc, time::Instant};
fn opened(native: bool) -> (App, PathBuf, PathBuf) {
    let dir = scratch();
    let path = dir.join("opened.pcnd");
    let editor = Editor::new_document(24, 16).unwrap();
    assert!(
        save::save_with(&path, false, save::STAGED_FILE_BYTES_MAX, |sink| {
            paintcraft_formats::pcnd::write_compact_to(editor.document(), sink)
        })
        .is_success()
    );
    let mut app = if native { app() } else { App::new(None) };
    app.open_path(&path).unwrap();
    (app, dir, path)
}
fn submit(app: &mut App, path: PathBuf) {
    app.saving
        .as_mut()
        .unwrap()
        .submit(
            &app.editor,
            path,
            app.bound.as_deref(),
            egui::Context::default(),
        )
        .unwrap();
}
fn edit(app: &mut App, id: u64) {
    app.editor.request(id, Action::AddLayer);
    app.editor.run_pending();
}
fn gate(app: &mut App) -> (mpsc::Sender<()>, Ticket) {
    let (release, waiting) = mpsc::channel();
    let ticket = app
        .saving
        .as_mut()
        .unwrap()
        .io
        .submit(
            1,
            move || {
                waiting.recv_timeout(Duration::from_secs(5)).unwrap();
                Ok(Completion::Cleanup)
            },
            || {},
        )
        .unwrap();
    (release, ticket)
}
fn release(app: &mut App, signal: mpsc::Sender<()>, expected: Ticket) {
    signal.send(()).unwrap();
    let deadline = Instant::now() + Duration::from_secs(5);
    loop {
        if let Some((ticket, outcome)) = app.saving.as_mut().unwrap().io.poll() {
            assert_eq!(ticket, expected);
            outcome.unwrap();
            break;
        }
        assert!(Instant::now() < deadline);
        std::thread::yield_now();
    }
    drain(app);
}
#[test]
fn queued_implicit_saves_advance_the_binding_before_ui_completion() {
    let (mut app, dir, path) = opened(true);
    let (signal, ticket) = gate(&mut app);
    edit(&mut app, 1);
    submit(&mut app, path.clone());
    edit(&mut app, 2);
    submit(&mut app, path.clone());
    release(&mut app, signal, ticket);
    assert!(!app.editor.is_dirty(), "{}", app.editor.status());
    assert!(
        Editor::open(&path)
            .unwrap()
            .document()
            .same_semantics(app.editor.document())
    );
    assert!(app.saving.as_ref().unwrap().attempts.is_empty());
    edit(&mut app, 3);
    fs::write(&path, b"external").unwrap();
    submit(&mut app, path.clone());
    drain(&mut app);
    assert_eq!(fs::read(&path).unwrap(), b"external");
    assert!(app.editor.is_dirty());
    fs::remove_dir_all(dir).unwrap();
}
#[test]
fn queued_conflicts_preserve_external_bytes_and_never_recreate_a_deleted_source() {
    for deleted in [false, true] {
        let (mut app, dir, path) = opened(true);
        let (signal, ticket) = gate(&mut app);
        edit(&mut app, 1);
        submit(&mut app, path.clone());
        edit(&mut app, 2);
        submit(&mut app, path.clone());
        if deleted {
            fs::remove_file(&path).unwrap();
        } else {
            fs::write(&path, b"external").unwrap();
        }
        release(&mut app, signal, ticket);
        assert!(app.editor.is_dirty());
        assert_eq!(app.saving.as_ref().unwrap().attempts.len(), 2);
        if deleted {
            assert!(!path.exists());
        } else {
            assert_eq!(fs::read(&path).unwrap(), b"external");
        }
        assert!(app.editor.status().contains("changed since input capture"));
        fs::remove_dir_all(dir).unwrap();
    }
}
#[test]
fn save_as_adopts_a_new_guard_without_mutating_the_original_association() {
    let (mut app, dir, original) = opened(true);
    let old = fs::read(&original).unwrap();
    let new = dir.join("new.pcnd");
    edit(&mut app, 1);
    submit(&mut app, new.clone());
    drain(&mut app);
    assert_eq!(app.bound.as_deref(), Some(new.as_path()));
    assert_eq!(fs::read(&original).unwrap(), old);
    edit(&mut app, 2);
    fs::write(&new, b"newer external").unwrap();
    submit(&mut app, new.clone());
    drain(&mut app);
    assert!(app.editor.is_dirty());
    assert_eq!(fs::read(&new).unwrap(), b"newer external");
    assert_eq!(fs::read(&original).unwrap(), old);
    fs::remove_dir_all(dir).unwrap();
}
#[test]
fn synchronous_failed_save_as_restores_the_original_expected_content() {
    let (mut app, dir, original) = opened(false);
    let ctx = egui::Context::default();
    edit(&mut app, 1);
    app.capture_save(&ctx, dir.join("missing/new.pcnd"));
    app.editor.run_pending();
    app.finish_save();
    assert_eq!(app.editor.path(), Some(original.as_path()));
    fs::write(&original, b"external").unwrap();
    app.save_activation = None;
    app.capture_save(&ctx, original.clone());
    app.editor.run_pending();
    app.finish_save();
    assert!(app.editor.is_dirty());
    assert_eq!(fs::read(&original).unwrap(), b"external");
    fs::remove_dir_all(dir).unwrap();
}
#[cfg(unix)]
#[test]
fn changed_open_alias_cannot_overwrite_an_equal_content_different_namespace() {
    for native in [false, true] {
        let (mut app, dir, original) = opened(native);
        let bytes = fs::read(&original).unwrap();
        let other = dir.join("other.pcnd");
        let alias = dir.join("alias.pcnd");
        fs::write(&other, &bytes).unwrap();
        std::os::unix::fs::symlink(&original, &alias).unwrap();
        app.open_path(&alias).unwrap();
        edit(&mut app, 1);
        fs::remove_file(&alias).unwrap();
        std::os::unix::fs::symlink(&other, &alias).unwrap();
        if native {
            submit(&mut app, alias.clone());
            drain(&mut app);
        } else {
            app.capture_save(&egui::Context::default(), alias);
            app.editor.run_pending();
            app.finish_save();
        }
        assert!(app.editor.is_dirty());
        assert!(
            app.editor.status().contains("namespace changed"),
            "{}",
            app.editor.status()
        );
        assert_eq!(fs::read(&original).unwrap(), bytes);
        assert_eq!(fs::read(&other).unwrap(), bytes);
        fs::remove_dir_all(dir).unwrap();
    }
}

#[test]
fn live_reconciliation_retains_the_guard_for_later_implicit_saves() {
    let (mut app, dir, path) = opened(true);
    edit(&mut app, 1);
    let snapshot = app.editor.document_snapshot();
    let charge = snapshot_charge(&snapshot).unwrap();
    let binding = app.editor.destination_binding().unwrap();
    let writer_binding = binding.clone();
    let document = snapshot.clone();
    let destination = path.clone();
    let saving = app.saving.as_mut().unwrap();
    let (session, generation) = (saving.session, saving.generation);
    let ticket = saving
        .submit_io_with_ticket(
            charge,
            move |ticket| {
                let mut completion = persistence::save_document(
                    &document,
                    &destination,
                    None,
                    paintcraft_formats::save_recovery::Stamp {
                        session,
                        generation,
                        sequence: ticket.sequence(),
                        operation: "save",
                    },
                    Some(&writer_binding),
                )?;
                let Completion::RetainedFile(file) = &mut completion else {
                    panic!("saved file")
                };
                assert!(file.outcome.is_success());
                // The adapter's acknowledged replacement remains, but the coordinator
                // needs to obtain a fresh durability confirmation before adoption.
                file.outcome.durable = false;
                file.outcome
                    .reasons
                    .push("injected missing durability confirmation".into());
                Ok(completion)
            },
            || {},
        )
        .unwrap();
    saving.pending.push(Pending {
        ticket,
        kind: PendingKind::Save(SaveRequest {
            binding: Some(binding),
            session,
            generation,
            source: snapshot.state_id,
            path: path.clone(),
            operation: "save",
            snapshot,
            charge,
        }),
    });
    drain(&mut app);
    assert!(app.editor.is_dirty());
    app.saving
        .as_mut()
        .unwrap()
        .request_reconciliation(ticket.sequence(), save::ReconcileMode::ConfirmDurability)
        .unwrap();
    drain(&mut app);
    assert!(!app.editor.is_dirty(), "{}", app.editor.status());
    edit(&mut app, 2);
    submit(&mut app, path.clone());
    drain(&mut app);
    assert!(!app.editor.is_dirty());
    assert!(
        Editor::open(&path)
            .unwrap()
            .document()
            .same_semantics(app.editor.document())
    );
    edit(&mut app, 3);
    fs::write(&path, b"external after reconciliation").unwrap();
    submit(&mut app, path.clone());
    drain(&mut app);
    assert!(app.editor.is_dirty());
    assert_eq!(fs::read(path).unwrap(), b"external after reconciliation");
    fs::remove_dir_all(dir).unwrap();
}

#[cfg(unix)]
#[test]
fn resource_adoption_refuses_a_source_targeted_by_an_already_queued_project_save() {
    use paintcraft_ui::{
        actions::Action as UiAction,
        i18n::{self, Language},
    };
    const MESSAGE: &str =
        "The resource source has a pending project save. Wait for it to finish and import again.";
    for shape in [false, true] {
        for other_tab in [false, true] {
            let dir = scratch();
            let mut app = app();
            app.settings.experience = Experience::Studio;
            let (extension, bytes): (_, &[u8]) = if shape {
                (
                    "csh",
                    include_bytes!(
                        "../../../../crates/services/paintcraft-formats/tests/csh-fixtures/paths.csh"
                    ),
                )
            } else {
                (
                    "ase",
                    include_bytes!(
                        "../../../../crates/services/paintcraft-formats/tests/ase-fixtures/mixed.ase"
                    ),
                )
            };
            let old = dir.join(format!("old.{extension}"));
            let source = dir.join(format!("new.{extension}"));
            let target = dir.join("chosen.pcnd");
            fs::write(&old, bytes).unwrap();
            fs::write(&source, bytes).unwrap();
            if shape {
                app.install_shape(old.clone()).unwrap();
            } else {
                app.install_palette(old.clone()).unwrap();
            }
            std::os::unix::fs::symlink(&source, &target).unwrap();
            let (signal, ticket) = gate(&mut app);
            let ctx = egui::Context::default();
            app.save_selected(&ctx, target.clone());
            let Some(Confirm::ReplaceFile(path)) = app.confirm.take() else {
                panic!("expected replacement review");
            };
            app.do_save(&ctx, path);
            if other_tab {
                app.add_document_tab(Editor::new_document(2, 2).unwrap(), &ctx)
                    .unwrap();
            }
            assert!(app.saving.as_ref().unwrap().pending());
            let action = if shape {
                UiAction::ShapeResources
            } else {
                UiAction::Palettes
            };
            assert!(
                action.enabled(app.action_context()),
                "UI does not globally gate imports on saves"
            );
            let mut results = Vec::new();
            for language in Language::ALL {
                i18n::set_language(&app.context, language);
                let adoption = if shape {
                    app.begin_shapes();
                    app.install_shape(source.clone())
                } else {
                    app.begin_palettes();
                    app.install_palette(source.clone())
                };
                results.push((language, adoption));
            }
            release(&mut app, signal, ticket);
            assert!(
                fs::read(&source).unwrap().starts_with(b"PCND"),
                "the earlier authorized save must complete"
            );
            for (language, result) in results {
                let error = result.expect_err(
                    "adopted a resource whose source the queued project save then replaced",
                );
                assert_eq!(error.code, ErrorCode::ConflictStaleBase);
                assert_eq!(error.message, i18n::translate(language, MESSAGE));
                if language != Language::English {
                    assert_ne!(error.message, MESSAGE);
                }
            }
            assert_eq!(
                app.asset_source_paths().collect::<Vec<_>>(),
                [old.as_path()]
            );
            assert_eq!(fs::read(&old).unwrap(), bytes);
            drop(app);
            fs::remove_dir_all(dir).unwrap();
        }
    }
}

#[test]
fn resource_adoption_remains_available_while_an_unrelated_project_save_is_queued() {
    for shape in [false, true] {
        let dir = scratch();
        let mut app = app();
        app.settings.experience = Experience::Studio;
        let (extension, bytes): (_, &[u8]) = if shape {
            (
                "csh",
                include_bytes!(
                    "../../../../crates/services/paintcraft-formats/tests/csh-fixtures/paths.csh"
                ),
            )
        } else {
            (
                "ase",
                include_bytes!(
                    "../../../../crates/services/paintcraft-formats/tests/ase-fixtures/mixed.ase"
                ),
            )
        };
        let source = dir.join(format!("unrelated.{extension}"));
        let target = dir.join("painting.pcnd");
        fs::write(&source, bytes).unwrap();
        let (signal, ticket) = gate(&mut app);
        let ctx = egui::Context::default();
        app.save_selected(&ctx, target.clone());
        let snapshot = app.editor.document_snapshot();
        let adoption = if shape {
            app.begin_shapes();
            app.install_shape(source.clone())
        } else {
            app.begin_palettes();
            app.install_palette(source.clone())
        };
        release(&mut app, signal, ticket);
        adoption.unwrap();
        assert_eq!(
            app.asset_source_paths().collect::<Vec<_>>(),
            [source.as_path()]
        );
        assert_eq!(fs::read(&source).unwrap(), bytes);
        assert!(fs::read(&target).unwrap().starts_with(b"PCND"));
        assert!(Arc::ptr_eq(&snapshot, &app.editor.document_snapshot()));
        drop(app);
        fs::remove_dir_all(dir).unwrap();
    }
}
