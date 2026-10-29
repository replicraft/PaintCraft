use super::*;

#[test]
#[ignore = "copied real public V8 installation only"]
fn fixture_real_export_review_and_guards() {
    export_review_and_guards(Oracle::C);
}
pub(super) fn export_review_and_guards(oracle: Oracle) {
    let root = directory().join(format!("jxr-export-guards-{}", uuid::Uuid::new_v4()));
    fs::create_dir(&root).unwrap();
    let manifest = resources_manifest_path();
    let original_manifest = fs::read(&manifest).unwrap();
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        fs::set_permissions(&manifest, fs::Permissions::from_mode(0o644)).unwrap();
    }
    for language in [Language::English, Language::Japanese] {
        for case in 0..7 {
            let source = root.join(format!("layered-{language:?}-{case}.pcnd"));
            let document = match oracle {
                Oracle::C => {
                    let converted =
                        paintcraft_formats::project_import::decode_ora(ORA, 256, &|| Ok(()))
                            .unwrap();
                    assert_eq!(converted.document.canvas, (3, 2));
                    converted.document
                }
                Oracle::Portable => portable::guard_document(),
            };
            let mut app = app(language, &source, document);
            let target = root.join(format!("output-{language:?}-{case}.jxr"));
            fs::write(&target, b"existing destination").unwrap();
            app.path_text = target.with_extension("pcnd").to_string_lossy().into_owned();
            let mut h = harness(app);
            let before = Before::capture(h.state());
            let metadata = h.state().editing.as_ref().unwrap().metadata_budget();
            let baseline = metadata.usage().reserved;
            choose(&mut h, language, &target);
            before.unchanged(h.state());
            assert_eq!(fs::read(&target).unwrap(), b"existing destination");
            let state = h.state().jxr_export.as_ref().unwrap();
            assert!(
                state
                    .prepared
                    .as_ref()
                    .unwrap()
                    .losses
                    .iter()
                    .any(|s| s == "flatten")
            );
            assert!(
                state
                    .prepared
                    .as_ref()
                    .unwrap()
                    .losses
                    .iter()
                    .any(|s| s == "hidden-rgb")
            );
            for message in [
                "Visible layers will be flattened into one picture.",
                "Invisible color information will not be retained.",
                "The file exists and will be replaced.",
            ] {
                assert!(
                    h.query_all_by_label_contains(&translate(language, message))
                        .count()
                        > 0,
                    "{language:?}: {message}"
                );
            }
            match case {
                0 => {
                    h.get_by_role_and_label(egui::Role::Button, &translate(language, "Export now"))
                        .click();
                    h.run();
                    assert!(h.state().jxr_export.is_none());
                    match oracle {
                        Oracle::C => reference(&target, 3, 2, EXPECTED),
                        Oracle::Portable => portable::verify_output(&target, 4, 3),
                    }
                    before.unchanged(h.state());
                }
                1 => {
                    h.get_by_role_and_label(egui::Role::Button, &translate(language, "Cancel"))
                        .click();
                    h.run();
                    before.unchanged(h.state());
                }
                2 => {
                    h.state_mut().editor.begin_text(Uuid::nil(), (0, 0));
                    h.run();
                    assert!(h.state().jxr_export.is_none());
                    assert!(h.state().editor.is_editing_text());
                    before.unchanged(h.state());
                }
                3 => {
                    h.state_mut().settings.experience = Experience::Classic;
                    h.run();
                    assert!(h.state().jxr_export.is_none());
                    before.unchanged(h.state());
                }
                4 => {
                    // Test the production picker completion seam while retaining
                    // the real prepared frame/lease; no OS picker is opened.
                    let state = h.state_mut().jxr_export.as_mut().unwrap();
                    state.destination = None;
                    h.state_mut().confirm = None;
                    h.state_mut().jxr_export_picker = true;
                    let moved = source.with_extension("renamed.pcnd");
                    fs::rename(&source, &moved).unwrap();
                    let alias = source.with_extension("alias.jxr");
                    fs::hard_link(&moved, &alias).unwrap();
                    let original = fs::read(&alias).unwrap();
                    let ctx = h.state().context.clone();
                    h.state_mut().complete_dialog(
                        &ctx,
                        paintcraft_platform::dialogs::Completion {
                            kind: Kind::Export,
                            path: Some(alias.clone()),
                        },
                    );
                    h.run();
                    assert!(h.state().jxr_export.is_none());
                    assert_eq!(fs::read(alias).unwrap(), original);
                    before.unchanged(h.state());
                }
                5 => {
                    let mut changed = original_manifest.clone();
                    changed.push(b' ');
                    fs::write(&manifest, changed).unwrap();
                    h.get_by_role_and_label(egui::Role::Button, &translate(language, "Export now"))
                        .click();
                    h.run();
                    fs::write(&manifest, &original_manifest).unwrap();
                    assert!(h.state().jxr_export.is_none());
                    before.unchanged(h.state());
                }
                6 => {
                    let ctx = h.state().context.clone();
                    h.state_mut()
                        .add_document_tab(Editor::new_document(4, 3).unwrap(), &ctx)
                        .unwrap();
                    let current = h.state().editor.document_snapshot();
                    h.run();
                    assert!(h.state().jxr_export.is_none());
                    assert!(Arc::ptr_eq(&current, &h.state().editor.document_snapshot()));
                    assert!(
                        h.state()
                            .tabs
                            .iter()
                            .filter_map(|tab| tab.state.as_ref())
                            .any(|s| Arc::ptr_eq(&before.doc, &s.editor.document_snapshot()))
                    );
                }
                _ => unreachable!(),
            }
            if case != 0 {
                assert_eq!(fs::read(&target).unwrap(), b"existing destination");
            }
            retire(h.state());
            assert_eq!(metadata.usage().reserved, baseline);
            println!(
                "Native export {language:?} case{case}: real worker, visible controls, source/state/publication guards"
            );
        }
    }
}

#[test]
#[ignore = "copied real public V8 installation only"]
fn fixture_real_fresh_long_output_import_and_source_guard() {
    let root = directory();
    for language in [Language::English, Language::Japanese] {
        let source = root.join(format!("wide-source-{language:?}.pcnd"));
        let doc = paintcraft_document::new_document(
            4096,
            1,
            Some(Rgba([255; 4])),
            0,
            paintcraft_document::POLICY,
        )
        .unwrap();
        let mut app = app(language, &source, doc);
        let target = root.join(format!("wide-copy-{language:?}.jxr"));
        app.path_text = target.with_extension("pcnd").to_string_lossy().into_owned();
        let mut h = harness(app);
        let before = Before::capture(h.state());
        choose(&mut h, language, &target);
        h.get_by_role_and_label(egui::Role::Button, &translate(language, "Export now"))
            .click();
        h.run();
        retire(h.state());
        before.unchanged(h.state());
        let bytes = fs::read(&target).unwrap();
        assert!(
            paintcraft_formats::jxr::assess_native_import(&bytes, 4096, &|| Ok(()))
                .unwrap()
                .is_writer_compatible()
        );
        let expected = vec![255; 4096 * 4];
        reference(&target, 4096, 1, &expected);
        h.state_mut().import_selected(target.clone());
        let until = Instant::now() + Duration::from_secs(15);
        loop {
            h.step();
            if h.query_all_by_role_and_label(
                egui::Role::CheckBox,
                &translate(language, "Accept import limitations"),
            )
            .count()
                > 0
            {
                break;
            }
            assert!(Instant::now() < until, "{}", h.state().editor.status());
            std::thread::sleep(Duration::from_millis(1));
        }
        before.unchanged(h.state());
        h.run();
        assert!(
            h.get_by_role_and_label(egui::Role::Button, &translate(language, "Open image"))
                .accesskit_node()
                .is_disabled()
        );
        h.get_by_role_and_label(
            egui::Role::CheckBox,
            &translate(language, "Accept import limitations"),
        )
        .click();
        h.run();
        h.get_by_role_and_label(egui::Role::Button, &translate(language, "Open image"))
            .click();
        h.run();
        assert!(
            h.state().pending_import.is_none(),
            "{}",
            h.state().editor.status()
        );
        pixels(h.state().editor.document(), 4096, 1, &expected);
        assert_eq!(h.state().document_count(), before.count + 1);
        let project = root.join(format!("wide-converted-{language:?}.pcnd"));
        let ctx = h.state().context.clone();
        h.state_mut().save_selected(&ctx, project.clone());
        let until = Instant::now() + Duration::from_secs(15);
        loop {
            h.state_mut().poll_edits(&ctx);
            h.state_mut().finish_save();
            if project.is_file()
                && !h.state().any_pending_edits()
                && !h.state().any_deferred_files()
            {
                break;
            }
            assert!(Instant::now() < until);
            std::thread::sleep(Duration::from_millis(1));
        }
        let reopened = paintcraft_formats::pcnd::read(&fs::read(project).unwrap()).unwrap();
        pixels(&reopened, 4096, 1, &expected);
        let moved = target.with_extension("renamed.jxr");
        fs::rename(&target, &moved).unwrap();
        let alias = target.with_extension("source-alias.pcnd");
        fs::hard_link(&moved, &alias).unwrap();
        h.state_mut().save_selected(&ctx, alias.clone());
        assert_eq!(fs::read(alias).unwrap(), bytes);
        retire(h.state());
        println!(
            "Native {language:?}: actual long-header UI export/C exact/selected import/PCND/source-alias guards"
        );
    }
}
