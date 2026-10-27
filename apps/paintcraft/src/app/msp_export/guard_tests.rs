use super::*;
use egui_kittest::kittest::NodeT;
#[test]
fn actual_options_default_to_v1_and_changing_review_version_retires_the_artifact() {
    for lang in [Language::English, Language::Japanese] {
        let root = root();
        let source = root.join("source.pcnd");
        let mut app = app(lang, &source, [255; 4]);
        app.path_text = root.join("copy.pcnd").to_string_lossy().into_owned();
        app.begin_export_options(paintcraft_ui::actions::ImageExport::Msp);
        let mut h = harness(app);
        click(&mut h, lang, "Preview export");
        prepared(&mut h);
        assert_eq!(
            h.state().msp_export.as_ref().unwrap().version,
            Version::UncompressedV1
        );
        let before = h
            .state()
            .msp_export
            .as_ref()
            .unwrap()
            .prepared
            .as_ref()
            .unwrap()
            .artifact
            .bytes()
            .to_vec();
        h.get_by_label(&translate(lang, "MSP version")).click();
        h.run();
        h.get_by_label(&translate(lang, "Version 2 (compressed)"))
            .click();
        h.run();
        prepared(&mut h);
        let state = h.state().msp_export.as_ref().unwrap();
        assert_eq!(state.version, Version::RowRleV2);
        assert!(state.destination.is_none());
        assert!(!state.losses_accepted);
        let after = state.prepared.as_ref().unwrap().artifact.bytes();
        assert_ne!(before, after);
        assert_eq!(
            msp::decode(after, &|| Ok(())).unwrap().header().version(),
            msp::Version::RowRleV2
        );
        drop(h);
        fs::remove_dir_all(root).unwrap();
    }
}
#[test]
fn actual_flatten_loss_requires_approval_and_no_compression_loss_is_invented() {
    for lang in [Language::English, Language::Japanese] {
        let root = root();
        let source = root.join("layered.pcnd");
        let mut doc = document([255; 4]);
        let mut layer = doc.layers[&doc.roots[0]].clone();
        layer.id = Uuid::new_v4();
        doc.roots.push(layer.id);
        doc.layers.insert(layer.id, layer);
        fs::write(&source, paintcraft_formats::pcnd::write(&doc).unwrap()).unwrap();
        let mut app = App::new(None);
        app.settings.experience = Experience::Studio;
        app.settings.language = lang;
        app.initialize_editing(app.context.clone());
        app.open_path(&source).unwrap();
        app.path_text = root.join("copy.pcnd").to_string_lossy().into_owned();
        app.begin_msp_export(Version::RowRleV2);
        let mut h = harness(app);
        prepared(&mut h);
        let state = h.state().msp_export.as_ref().unwrap();
        assert_eq!(
            state.prepared.as_ref().unwrap().artifact.losses(),
            ["flatten"]
        );
        assert!(!state.losses_accepted);
        let label = translate(lang, "Choose destination…");
        let choose = h.get_by_role_and_label(egui::Role::Button, &label);
        assert!(choose.accesskit_node().is_disabled());
        h.get_by_role_and_label(
            egui::Role::CheckBox,
            &translate(lang, "I accept these export changes."),
        )
        .click();
        h.run();
        click(&mut h, lang, "Choose destination…");
        click(&mut h, lang, "Export now");
        assert!(root.join("copy.msp").exists());
        assert!(h.state().editor.document().same_semantics(&doc));
        drop(h);
        fs::remove_dir_all(root).unwrap();
    }
}
#[test]
fn delayed_picker_preserves_an_unrelated_newer_modal() {
    let root = root();
    let source = root.join("source.pcnd");
    let mut app = app(Language::English, &source, [255; 4]);
    app.begin_msp_export(Version::RowRleV2);
    let mut h = harness(app);
    prepared(&mut h);
    let newer = Confirm::DiscardForNew;
    h.state_mut().confirm = Some(newer.clone());
    h.state_mut()
        .complete_msp_picker(Some(root.join("late.msp")));
    assert_eq!(h.state().confirm, Some(newer));
    assert!(h.state().msp_export.is_none());
    assert!(!root.join("late.msp").exists());
    drop(h);
    fs::remove_dir_all(root).unwrap();
}
#[test]
fn post_stage_renamed_source_alias_is_refused_before_publication() {
    let root = root();
    let source = root.join("source.pcnd");
    let target = root.join("copy.msp");
    let mut app = app(Language::English, &source, [255; 4]);
    let bytes = fs::read(&source).unwrap();
    app.path_text = target.with_extension("pcnd").to_string_lossy().into_owned();
    app.begin_msp_export(Version::RowRleV2);
    let mut h = harness(app);
    prepared(&mut h);
    click(&mut h, Language::English, "Choose destination…");
    let moved = root.join("moved.pcnd");
    let source_for_hook = source.clone();
    let moved_for_hook = moved.clone();
    let target_for_hook = target.clone();
    BEFORE_PUBLICATION.with(|hook| {
        *hook.borrow_mut() = Some(Box::new(move || {
            fs::rename(source_for_hook, &moved_for_hook).unwrap();
            fs::hard_link(moved_for_hook, target_for_hook).unwrap();
        }))
    });
    let snapshot = h.state().editor.document_snapshot();
    let ticket = h.state().editor.history_view().ticket;
    let dirty = h.state().editor.is_dirty();
    click(&mut h, Language::English, "Export now");
    assert_eq!(fs::read(&target).unwrap(), bytes);
    assert_eq!(fs::read(&moved).unwrap(), bytes);
    assert!(h.state().editor.status().contains("Cannot export"));
    assert_eq!(ticket, h.state().editor.history_view().ticket);
    assert_eq!(dirty, h.state().editor.is_dirty());
    assert!(Arc::ptr_eq(
        &snapshot,
        &h.state().editor.document_snapshot()
    ));
    drop(h);
    fs::remove_dir_all(root).unwrap();
}
#[test]
fn original_msp_identity_remains_protected_after_native_project_save() {
    let root = root();
    let source = root.join("original.msp");
    let bytes = msp::encode_v1_rgba_exact(7, 5, &[255; 7 * 5 * 4], &|| Ok(())).unwrap();
    fs::write(&source, &bytes).unwrap();
    let mut app = App::new(None);
    app.settings.experience = Experience::Studio;
    app.initialize_editing(app.context.clone());
    let acquired =
        crate::format_admission::acquire_resource_source(&source, msp::BYTES_MAX as u64).unwrap();
    app.stage_msp_import_acquired(acquired, source.clone())
        .unwrap();
    app.resolve_import(&egui::Context::default(), true);
    let mut h = harness(app);
    let pcnd = root.join("saved.pcnd");
    let ctx = h.ctx.clone();
    h.state_mut().save_selected(&ctx, pcnd.clone());
    let until = Instant::now() + Duration::from_secs(10);
    while h.state().bound.as_deref() != Some(pcnd.as_path()) {
        h.step();
        assert!(Instant::now() < until, "{}", h.state().editor.status());
        std::thread::sleep(Duration::from_millis(1));
    }
    let moved = root.join("renamed.msp");
    fs::rename(&source, &moved).unwrap();
    let target = root.join("alias.msp");
    fs::hard_link(&moved, &target).unwrap();
    h.state_mut().path_text = target.with_extension("pcnd").to_string_lossy().into_owned();
    h.state_mut().begin_msp_export(Version::RowRleV2);
    h.run();
    prepared(&mut h);
    click(&mut h, Language::English, "Choose destination…");
    assert!(h.state().msp_export.is_none());
    assert_eq!(fs::read(&target).unwrap(), bytes);
    assert_eq!(h.state().bound.as_deref(), Some(pcnd.as_path()));
    drop(h);
    fs::remove_dir_all(root).unwrap();
}

#[test]
fn actual_native_copy_matches_the_independent_msp2_run_packet_golden() {
    const GOLDEN: &[u8] = include_bytes!(
        "../../../../../crates/services/paintcraft-formats/tests/msp2-writer-fixtures/runs-2048.msp"
    );
    let bitmap = msp::decode(GOLDEN, &|| Ok(())).unwrap();
    let rgba = bitmap.rgba(&|| Ok(())).unwrap();
    let mut doc = paintcraft_document::new_document(
        2048,
        3,
        Some(Rgba([0, 0, 0, 255])),
        0,
        paintcraft_document::POLICY,
    )
    .unwrap();
    doc.layers.get_mut(&doc.roots[0]).unwrap().raster = paintcraft_raster::RasterLayer::from_rgba(
        2048,
        3,
        Rgba([0; 4]),
        &rgba,
        &mut paintcraft_raster::Admission::new(8 << 20),
    )
    .unwrap();
    let root = root();
    let source = root.join("runs.pcnd");
    fs::write(&source, paintcraft_formats::pcnd::write(&doc).unwrap()).unwrap();
    let mut app = App::new(None);
    app.settings.experience = Experience::Studio;
    app.initialize_editing(app.context.clone());
    app.open_path(&source).unwrap();
    app.path_text = root.join("runs-copy.pcnd").to_string_lossy().into_owned();
    app.begin_export_options(paintcraft_ui::actions::ImageExport::Msp);
    let mut h = harness(app);
    h.get_by_label("MSP version").click();
    h.run();
    h.get_by_label("Version 2 (compressed)").click();
    h.run();
    click(&mut h, Language::English, "Preview export");
    prepared(&mut h);
    assert_eq!(
        h.state().msp_export.as_ref().unwrap().version,
        Version::RowRleV2
    );
    click(&mut h, Language::English, "Choose destination…");
    click(&mut h, Language::English, "Export now");
    let saved = fs::read(root.join("runs-copy.msp")).unwrap();
    assert_eq!(saved, GOLDEN);
    if let Some(directory) = std::env::var_os("PAINTCRAFT_MSP_UI_EVIDENCE") {
        fs::create_dir_all(&directory).unwrap();
        fs::write(Path::new(&directory).join("runs-copy.msp"), saved).unwrap();
    }
    drop(h);
    fs::remove_dir_all(root).unwrap();
}
