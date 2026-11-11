//! Requires the same real copied public installation as sibling installed cases.
use super::*;
#[test]
#[ignore = "copied real qualified writer installation; not a unit fixture grant"]
fn scoped_generation_exact_service_capacity_adopts_and_staged_aba_preserves_source_and_destination()
{
    let root = directory().join(format!("jxr-generation-{}", Uuid::new_v4()));
    fs::create_dir(&root).unwrap();
    let source = root.join("source.pcnd");
    let target = root.join("copy.jxr");
    let mut app = app(Language::English, &source, portable::guard_document());
    app.worker_budget = paintcraft_runtime::SharedBudget::new(2 << 30);
    app.initialize_editing(app.context.clone());
    assert_eq!(app.worker_budget.usage().limit, 2 << 30);
    app.path_text = target.with_extension("pcnd").to_string_lossy().into_owned();
    fs::write(&target, b"old destination").unwrap();
    let mut h = harness(app);
    let before = Before::capture(h.state());
    choose(&mut h, Language::English, &target);
    let state = h.state_mut().jxr_export.take().unwrap();
    assert!(state.adopted);
    let ticket = state.ticket.unwrap();
    let prepared = state.prepared.as_ref().unwrap();
    assert_eq!(
        prepared.encoded.installation(),
        ticket.observation().identity().unwrap()
    );
    let shared = h.state().jxr_generation.clone();
    let outcome = save::save_with_expectation_and_preparation(
        &target,
        true,
        encode::ENCODED_BYTES_MAX,
        state.destination.as_ref().unwrap().1.as_ref(),
        |sink| {
            std::io::Write::write_all(sink, prepared.encoded.bytes()).unwrap();
            // An observed revoked environment followed by identical available bytes
            // cannot revive the old request. No unqualified helper is launched.
            let mut g = shared.lock().unwrap();
            let r = g.begin_refresh().unwrap();
            assert!(
                g.complete(r, installation::Observation::Unavailable)
                    .is_err()
            );
            let r = g.begin_refresh().unwrap();
            g.complete(r, ticket.observation()).unwrap();
            Ok(())
        },
        |_| generation::reobserve(&shared, ticket, &|| Ok(())),
    );
    assert!(!outcome.is_success());
    assert_eq!(fs::read(&target).unwrap(), b"old destination");
    before.unchanged(h.state());
    drop(state);
    retire(h.state());
    assert_eq!(h.state().worker_budget.usage().reserved, 0);
    fs::remove_dir_all(root).unwrap();
}
#[test]
#[ignore = "copied real qualified writer installation; execute serially"]
fn observed_revocation_after_preview_blocks_export_without_changing_document_or_destination() {
    let root = directory().join(format!("jxr-adoption-{}", Uuid::new_v4()));
    fs::create_dir(&root).unwrap();
    let source = root.join("source.pcnd");
    let target = root.join("copy.jxr");
    let mut app = app(Language::English, &source, portable::guard_document());
    app.path_text = target.with_extension("pcnd").to_string_lossy().into_owned();
    fs::write(&target, b"old destination").unwrap();
    let mut h = harness(app);
    let before = Before::capture(h.state());
    choose(&mut h, Language::English, &target);
    let ticket = h.state().jxr_export.as_ref().unwrap().ticket.unwrap();
    {
        let mut g = h.state().jxr_generation.lock().unwrap();
        let r = g.begin_refresh().unwrap();
        assert!(
            g.complete(r, installation::Observation::Unavailable)
                .is_err()
        );
    }
    assert!(generation::require(&h.state().jxr_generation, ticket).is_err());
    h.state_mut().finish_jxr_export();
    assert!(h.state().jxr_export.is_none());
    assert_eq!(fs::read(&target).unwrap(), b"old destination");
    before.unchanged(h.state());
    retire(h.state());
    fs::remove_dir_all(root).unwrap();
}

#[test]
#[ignore = "copied real qualified writer installation; execute serially"]
fn revoked_queued_export_refuses_before_render_and_releases_same_original_admission() {
    let root = directory().join(format!("jxr-queue-{}", Uuid::new_v4()));
    fs::create_dir(&root).unwrap();
    let source_path = root.join("source.pcnd");
    let target = root.join("copy.jxr");
    let app = app(Language::English, &source_path, portable::guard_document());
    fs::write(&target, b"old destination").unwrap();
    let before = Before::capture(&app);
    let shared = app.jxr_generation.clone();
    let observation = installation::observe(&|| Ok(())).unwrap();
    observation.identity().unwrap();
    let ticket = {
        let mut g = shared.lock().unwrap();
        let r = g.begin_refresh().unwrap();
        g.complete(r, observation).unwrap()
    };
    let source = app.editor.document_snapshot();
    let owner = app.editor.native_import_owner();
    let service = app.editing.as_ref().unwrap();
    let (started, wait_started) = std::sync::mpsc::channel();
    let (release, wait_release) = std::sync::mpsc::channel();
    let worker_generation = shared.clone();
    let pending = service
        .try_submit_jxr_export(1, move |cancel| {
            started.send(()).unwrap();
            wait_release.recv_timeout(Duration::from_secs(5)).unwrap();
            worker::prepare_with_generation(
                source,
                owner,
                cancel,
                export::CopyPolicies::default(),
                worker_generation,
                ticket,
            )
        })
        .unwrap()
        .unwrap();
    wait_started.recv_timeout(Duration::from_secs(5)).unwrap();
    {
        let mut g = shared.lock().unwrap();
        let r = g.begin_refresh().unwrap();
        assert!(
            g.complete(r, installation::Observation::Unavailable)
                .is_err()
        );
    }
    release.send(()).unwrap();
    let until = Instant::now() + Duration::from_secs(5);
    loop {
        if let Some(result) = pending.poll() {
            assert_eq!(result.unwrap_err().code, ErrorCode::ConflictStaleBase);
            break;
        }
        assert!(Instant::now() < until);
        std::thread::sleep(Duration::from_millis(1));
    }
    before.unchanged(&app);
    assert_eq!(fs::read(&target).unwrap(), b"old destination");
    drop(pending);
    retire(&app);
    assert_eq!(app.worker_budget.usage().reserved, 0);
    fs::remove_dir_all(root).unwrap();
}

#[test]
#[ignore = "same copied CLI and App beside real qualified writer; execute serially"]
fn adopted_gui_ticket_equals_actual_copied_cli_installation_record() {
    let root = directory().join(format!("jxr-discovery-{}", Uuid::new_v4()));
    fs::create_dir(&root).unwrap();
    let source = root.join("source.pcnd");
    let target = root.join("copy.jxr");
    let mut app = app(Language::English, &source, portable::guard_document());
    app.path_text = target.with_extension("pcnd").to_string_lossy().into_owned();
    fs::write(&target, b"old destination").unwrap();
    let mut h = harness(app);
    let before = Before::capture(h.state());
    choose(&mut h, Language::English, &target);
    let state = h.state().jxr_export.as_ref().unwrap();
    assert!(state.adopted);
    let gui = state.ticket.unwrap().observation().to_value();
    let cli = directory().join(if cfg!(windows) {
        "paintcraft-cli.exe"
    } else {
        "paintcraft-cli"
    });
    let result = std::process::Command::new(cli)
        .args(["capabilities", "--installation-jxr", "--json"])
        .output()
        .unwrap();
    assert!(
        result.status.success(),
        "{}",
        String::from_utf8_lossy(&result.stderr)
    );
    let actual = paintcraft_core::types::json::parse(&result.stdout).unwrap();
    assert_eq!(gui, actual);
    println!(
        "adopted GUI record={}",
        String::from_utf8(paintcraft_core::types::json::canonical(&gui)).unwrap()
    );
    println!(
        "actual copied CLI record={}",
        String::from_utf8(result.stdout).unwrap()
    );
    before.unchanged(h.state());
    h.state_mut().jxr_export = None;
    retire(h.state());
    assert_eq!(fs::read(&target).unwrap(), b"old destination");
    fs::remove_dir_all(root).unwrap();
}
#[test]
#[ignore = "real copied manifest is physically moved during staging; execute serially"]
fn actual_manifest_removed_during_staging_refuses_publication_and_restores_installation() {
    use std::cell::Cell;
    struct Restore {
        manifest: PathBuf,
        backup: PathBuf,
        moved: Cell<bool>,
    }
    impl Drop for Restore {
        fn drop(&mut self) {
            if self.moved.get() {
                fs::rename(&self.backup, &self.manifest).expect("restore owned copied manifest");
            }
        }
    }
    let root = directory().join(format!("jxr-staged-revoke-{}", Uuid::new_v4()));
    fs::create_dir(&root).unwrap();
    let source = root.join("source.pcnd");
    let target = root.join("copy.jxr");
    let mut app = app(Language::English, &source, portable::guard_document());
    app.path_text = target.with_extension("pcnd").to_string_lossy().into_owned();
    fs::write(&target, b"old destination").unwrap();
    let mut h = harness(app);
    let before = Before::capture(h.state());
    choose(&mut h, Language::English, &target);
    let state = h.state_mut().jxr_export.take().unwrap();
    assert!(state.adopted);
    let prepared = state.prepared.as_ref().unwrap();
    let ticket = state.ticket.unwrap();
    let restore = Restore {
        manifest: resources_manifest_path(),
        backup: root.join("held-manifest.json"),
        moved: Cell::new(false),
    };
    let manifest_bytes = fs::read(&restore.manifest).unwrap();
    let shared = h.state().jxr_generation.clone();
    let outcome = save::save_with_expectation_and_preparation(
        &target,
        true,
        encode::ENCODED_BYTES_MAX,
        state.destination.as_ref().unwrap().1.as_ref(),
        |sink| {
            std::io::Write::write_all(sink, prepared.encoded.bytes()).unwrap();
            fs::rename(&restore.manifest, &restore.backup).unwrap();
            restore.moved.set(true);
            Ok(())
        },
        |_| {
            generation::reobserve(&shared, ticket, &|| Ok(()))?;
            prepared
                .encoded
                .check_current(&|| Ok(()), &|| Ok(h.state().editor.native_import_owner()))
        },
    );
    assert!(restore.moved.get());
    let manifest = restore.manifest.clone();
    drop(restore);
    assert!(!outcome.is_success());
    assert_eq!(fs::read(&target).unwrap(), b"old destination");
    assert_eq!(fs::read(manifest).unwrap(), manifest_bytes);
    before.unchanged(h.state());
    drop(state);
    retire(h.state());
    assert_eq!(h.state().worker_budget.usage().reserved, 0);
    fs::remove_dir_all(root).unwrap();
}
#[test]
#[ignore = "real successful preparation paused before actual UI adoption; execute serially"]
fn successful_returned_result_with_observed_aba_is_refused_before_ui_adoption() {
    let root = directory().join(format!("jxr-returned-result-{}", Uuid::new_v4()));
    fs::create_dir(&root).unwrap();
    let source_path = root.join("source.pcnd");
    let target = root.join("copy.jxr");
    let mut app = app(Language::English, &source_path, portable::guard_document());
    app.path_text = target.with_extension("pcnd").to_string_lossy().into_owned();
    fs::write(&target, b"old destination").unwrap();
    let before = Before::capture(&app);
    let frozen = export_options::Frozen::capture(&app).unwrap();
    app.begin_jxr_export_frozen(frozen, Intent::Refuse);
    let until = Instant::now() + Duration::from_secs(10);
    let mut state = app.jxr_export.take().unwrap();
    while state.ticket.is_none() {
        generation::poll_refresh(&app.jxr_generation, &mut state).unwrap();
        assert!(Instant::now() < until);
        std::thread::sleep(Duration::from_millis(1));
    }
    // Initial observation is consumed/dropped before arming the existing real
    // completion gate. Do not accidentally pause observation instead of encoding.
    let ticket = state.ticket.unwrap();
    let capture = state.capture.as_ref().unwrap();
    let source = capture.source.clone();
    let owner = capture.owner;
    let shared = app.jxr_generation.clone();
    let worker_shared = shared.clone();
    let layout =
        encode::Layout::new(source.canvas.0, source.canvas.1, encode::ENCODED_BYTES_MAX).unwrap();
    let spatial = spatial_workspace::needed(&source);
    let service = app.editing.as_ref().unwrap();
    let (completed, release) = service.pause_next_completion();
    let (success, successful) = std::sync::mpsc::channel();
    state.pending = Some(
        service
            .try_submit_jxr_export_with_workspace(
                worker::quote(layout).unwrap(),
                move |cancel| {
                    let prepared = worker::prepare_with_generation(
                        source,
                        owner,
                        cancel,
                        export::CopyPolicies::default(),
                        worker_shared,
                        ticket,
                    )?;
                    assert!(!prepared.encoded.bytes().is_empty());
                    assert!(prepared.preview.is_some());
                    assert_eq!(
                        prepared.encoded.installation(),
                        ticket.observation().identity().unwrap()
                    );
                    success.send(()).unwrap();
                    Ok(prepared)
                },
                spatial,
            )
            .unwrap()
            .unwrap(),
    );
    app.jxr_export = Some(state);
    successful.recv_timeout(Duration::from_secs(15)).unwrap();
    completed.recv_timeout(Duration::from_secs(5)).unwrap();
    assert!(app.worker_budget.usage().reserved > 0);
    {
        let mut g = shared.lock().unwrap();
        let r = g.begin_refresh().unwrap();
        assert!(
            g.complete(r, installation::Observation::Unavailable)
                .is_err()
        );
        let r = g.begin_refresh().unwrap();
        g.complete(r, ticket.observation()).unwrap();
    }
    release.send(()).unwrap();
    let until = Instant::now() + Duration::from_secs(10);
    let ctx = app.context.clone();
    while app.jxr_export.is_some() {
        app.poll_jxr_export(&ctx);
        if let Some(state) = &app.jxr_export {
            assert!(state.texture.is_none());
            assert!(!state.adopted);
            assert!(!state.reviewed);
        }
        assert!(Instant::now() < until);
        std::thread::sleep(Duration::from_millis(1));
    }
    before.unchanged(&app);
    assert_eq!(fs::read(&target).unwrap(), b"old destination");
    retire(&app);
    assert_eq!(app.worker_budget.usage().reserved, 0);
    fs::remove_dir_all(root).unwrap();
}
