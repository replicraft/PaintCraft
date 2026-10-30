//! Actual same-installation UI: complement + one Invert equals pinned C golden.
use super::*;
const BAKE: &str = "Bake editable filters into this exported copy";
const OMIT: &str = "Omit original source metadata from this exported copy";
const RAW: &[u8] =
    include_bytes!("../../../../../apps/paintcraft-cli/tests/jxr-export-fixtures/4x3.rgba");
fn document() -> paintcraft_document::Document {
    let complement: Vec<u8> = RAW
        .as_chunks::<4>()
        .0
        .iter()
        .flat_map(|p| [255 - p[0], 255 - p[1], 255 - p[2], p[3]])
        .collect();
    let mut doc =
        paintcraft_document::import_image(4, 3, &complement, "jxr-bake-independent-complement/v1")
            .unwrap();
    doc.layers.get_mut(&doc.roots[0]).unwrap().filters =
        paintcraft_document::FilterStack::new(vec![paintcraft_document::FilterInstance::new(
            Uuid::from_u128(1),
            true,
            paintcraft_core::types::filters::Descriptor::InvertRgbV1,
        )])
        .unwrap();
    doc.source_metadata=paintcraft_formats::pcnd::read(include_bytes!("../../../../../crates/services/paintcraft-formats/tests/pcnd-metadata-fixtures/rgb-profile.pcnd")).unwrap().source_metadata;
    assert_eq!(paintcraft_render::render_flat(&doc).unwrap(), RAW);
    doc
}
fn begin(h: &mut Harness<App>, language: Language) {
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
    // Both choices are false, so this stage cannot start a worker or spinner.
    // Settle the modal's layout and the handoff click before the next input.
    h.run();
    let state = h.state().jxr_export.as_ref().unwrap();
    assert!(state.current(h.state()));
    assert!(!state.omit_source_metadata && !state.bake_filters);
    assert!(state.pending.is_none() && state.prepared.is_none());
    assert_eq!(
        state.capture.as_ref().unwrap().stamp,
        h.state().editor.preview_stamp()
    );
    assert!(state.frozen.as_ref().unwrap().current(h.state()));
    assert!(Arc::ptr_eq(
        &state.capture.as_ref().unwrap().source,
        &state.frozen.as_ref().unwrap().document
    ));
    assert_eq!(h.state().worker_budget.usage().reserved, 0);
    h.get_by_label(&translate(language, OMIT)).click();
    h.run();
    let state = h.state().jxr_export.as_ref().unwrap();
    assert!(state.omit_source_metadata && !state.bake_filters);
    assert!(state.pending.is_none() && state.prepared.is_none());
    assert_eq!(
        h.state().worker_budget.usage().reserved,
        0,
        "Metadata omission cannot authorize baking"
    );
    println!("{language:?}: metadata-only stage flushed, awaiting explicit bake preview");
    h.get_by_label(&translate(language, BAKE)).click();
    let until = Instant::now() + Duration::from_secs(15);
    while !h.state().jxr_export.as_ref().unwrap().bake_filters {
        h.step();
        assert!(Instant::now() < until, "Bake interaction was not consumed");
    }
    let state = h.state().jxr_export.as_ref().unwrap();
    assert!(state.current(h.state()) && state.metadata_ready());
    assert_eq!(
        state.capture.as_ref().unwrap().stamp,
        h.state().editor.preview_stamp()
    );
    assert_eq!(
        state
            .capture
            .as_ref()
            .unwrap()
            .source
            .layers
            .values()
            .flat_map(|l| l.filters.iter())
            .next()
            .unwrap()
            .descriptor(),
        paintcraft_core::types::filters::Descriptor::InvertRgbV1
    );
    wait_preview(h, language);
    let state = h.state().jxr_export.as_ref().unwrap();
    assert!(
        state
            .filter_bake
            .validate(
                &state.capture.as_ref().unwrap().source,
                Target::new(Family::Jxr, Operation::ExportFlat),
                &|| Ok(())
            )
            .unwrap()
    );
    assert_eq!(
        state
            .prepared
            .as_ref()
            .unwrap()
            .losses
            .iter()
            .filter(|s| s.as_str() == export::filter_bake::BAKED_REASON)
            .count(),
        1
    );
    h.get_by_role_and_label(
        egui::Role::Button,
        &translate(language, "Choose destination…"),
    )
    .click();
    h.step();
    wait_review(h, language);
}
#[test]
#[ignore = "requires a verified public writer helper beside the copied App test binary"]
fn fixture_installed_filter_bake_matches_c_golden_and_revoked_intent_cannot_publish() {
    for language in [Language::English, Language::Japanese] {
        let root = directory().join(format!("jxr-filter-bake-{}", Uuid::new_v4()));
        fs::create_dir(&root).unwrap();
        let source = root.join("original.pcnd");
        let target = root.join("copy.jxr");
        let mut app = app(language, &source, document());
        app.path_text = target.with_extension("pcnd").to_string_lossy().into_owned();
        let original = fs::read(&source).unwrap();
        assert_eq!(original[4], 3);
        fs::write(&target, b"existing destination").unwrap();
        let mut h = harness(app);
        let before = Before::capture(h.state());
        println!("{language:?}: first begin for final-intent revocation");
        begin(&mut h, language);
        println!("{language:?}: first real preview/review ready");
        // Retain the otherwise valid original witness/artifact while revoking the
        // current intent. Final publication must check intent independently.
        h.state_mut().jxr_export.as_mut().unwrap().bake_filters = false;
        h.state_mut().finish_jxr_export();
        retire(h.state());
        assert_eq!(fs::read(&target).unwrap(), b"existing destination");
        assert_eq!(fs::read(&source).unwrap(), original);
        before.unchanged(h.state());
        println!("{language:?}: second begin for positive publication");
        begin(&mut h, language);
        before.unchanged(h.state());
        h.get_by_role_and_label(egui::Role::Button, &translate(language, "Export now"))
            .click();
        h.run();
        retire(h.state());
        portable::verify_output(&target, 4, 3);
        before.unchanged(h.state());
        assert_eq!(fs::read(&source).unwrap(), original);
        let preserved =
            paintcraft_formats::pcnd::write_compact(h.state().editor.document()).unwrap();
        assert_eq!(preserved[4], 3);
        assert!(
            paintcraft_formats::pcnd::read(&preserved)
                .unwrap()
                .same_semantics(h.state().editor.document())
        );
        println!(
            "installed filter bake {language:?}: actual filtered preview, revoked-intent sentinel refusal, exact independently C-qualified4x3 output, source/history/metadata unchanged"
        );
    }
}
