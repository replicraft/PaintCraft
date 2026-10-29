//! Positive privacy delivery through a real installed helper and publication.
use super::*;
#[test]
#[ignore = "requires a verified public writer helper beside the copied App test binary"]
fn fixture_installed_opaque_metadata_omission_preserves_project_and_matches_c_goldens() {
    for language in [Language::English, Language::Japanese] {
        let root = directory().join(format!("jxr-metadata-{}", Uuid::new_v4()));
        fs::create_dir(&root).unwrap();
        let source = root.join("original.pcnd");
        let target = root.join("copy.jxr");
        let metadata = paintcraft_formats::pcnd::read(include_bytes!("../../../../../crates/services/paintcraft-formats/tests/pcnd-metadata-fixtures/rgb-profile.pcnd")).unwrap().source_metadata;
        let document = portable::guard_document()
            .with_source_metadata_checked(metadata, &|| Ok(()))
            .unwrap();
        let mut app = app(language, &source, document);
        let original = fs::read(&source).unwrap();
        app.path_text = target.with_extension("pcnd").to_string_lossy().into_owned();
        fs::write(&target, b"existing destination").unwrap();
        let mut h = harness(app);
        let before = Before::capture(h.state());
        h.state_mut()
            .begin_export_options(paintcraft_ui::actions::ImageExport::Png);
        h.run();
        h.get_by_role_and_label(egui::Role::ComboBox, &translate(language, "File format"))
            .click();
        h.run();
        h.get_by_label(&translate(language, "JPEG XR picture"))
            .scroll_to_me();
        h.run();
        h.get_by_label(&translate(language, "JPEG XR picture"))
            .click();
        h.run();
        h.get_by_role_and_label(egui::Role::Button, &translate(language, "Preview export"))
            .click();
        h.run();
        let state = h.state().jxr_export.as_ref().unwrap();
        assert!(state.pending.is_none());
        assert!(state.prepared.is_none());
        assert!(Arc::ptr_eq(
            &state.capture.as_ref().unwrap().source,
            &state.frozen.as_ref().unwrap().document
        ));
        assert_eq!(h.state().worker_budget.usage().reserved, 0);
        assert_eq!(fs::read(&target).unwrap(), b"existing destination");
        before.unchanged(h.state());
        h.get_by_role_and_label(
            egui::Role::CheckBox,
            &translate(
                language,
                "Omit original source metadata from this exported copy",
            ),
        )
        .click();
        h.step();
        wait_preview(&mut h, language);
        h.get_by_role_and_label(
            egui::Role::Button,
            &translate(language, "Choose destination…"),
        )
        .click();
        h.step();
        wait_review(&mut h, language);
        let state = h.state().jxr_export.as_ref().unwrap();
        preview_refusals::assert_bound_preview_refusals(
            &state.prepared.as_ref().unwrap().encoded,
            state.capture.as_ref().unwrap().owner,
        );
        assert_eq!(
            state
                .prepared
                .as_ref()
                .unwrap()
                .losses
                .iter()
                .filter(|s| s.as_str() == export::metadata_policy::OMITTED_REASON)
                .count(),
            1
        );
        state
            .metadata_policy
            .validate(
                &state.capture.as_ref().unwrap().source,
                Target::new(Family::Jxr, Operation::ExportFlat),
            )
            .unwrap();
        before.unchanged(h.state());
        assert_eq!(fs::read(&source).unwrap(), original);
        assert_eq!(fs::read(&target).unwrap(), b"existing destination");
        h.get_by_role_and_label(egui::Role::Button, &translate(language, "Export now"))
            .click();
        h.run();
        retire(h.state());
        before.unchanged(h.state());
        portable::verify_output(&target, 4, 3);
        assert_eq!(fs::read(&source).unwrap(), original);
        assert_eq!(
            h.state().editor.document().source_metadata,
            before.doc.source_metadata
        );
        let preserved =
            paintcraft_formats::pcnd::write_compact(h.state().editor.document()).unwrap();
        assert_eq!(preserved[4], 2);
        assert_eq!(
            paintcraft_formats::pcnd::read(&preserved)
                .unwrap()
                .source_metadata,
            before.doc.source_metadata
        );
        println!(
            "installed metadata omission {:?}: exact independently C-qualified 4x3 output; source, metadata, history and binding unchanged",
            language
        );
    }
}
