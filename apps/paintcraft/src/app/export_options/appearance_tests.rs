//! The real native export modal requires a fresh compound witness for styled PNG.
use super::*;
use egui_kittest::{Harness, kittest::Queryable};
use paintcraft_commands::{LayerRef, StyleOp};
use paintcraft_core::types::styles::{ColorOverlayRgbV1, EffectBlendMode, StyleDescriptor};
use std::time::Instant;
fn styled() -> App {
    let mut app = App::new(None);
    app.settings.experience = Experience::Studio;
    app.editor = Editor::with_document(
        paintcraft_document::import_image(
            2,
            1,
            &[10, 20, 30, 255, 100, 150, 200, 255],
            "style-native-png/v1",
        )
        .unwrap(),
        None,
    );
    app.initialize_editing(app.context.clone());
    let layer = app.editor.target().unwrap();
    let edit = app.editor.style_capture(StyleOp::Add {
        layer: LayerRef::Id(layer),
        alias: "overlay".into(),
        descriptor: StyleDescriptor::ColorOverlayRgbV1(ColorOverlayRgbV1::new(
            [255, 0, 0],
            255,
            EffectBlendMode::Normal,
        )),
        enabled: true,
        to_index: None,
    });
    app.editor.queue_style_apply(1001, edit).unwrap();
    let deadline = Instant::now() + Duration::from_secs(10);
    while app.editor.has_pending_edits() {
        app.editor.run_pending();
        assert!(Instant::now() < deadline);
        std::thread::yield_now();
    }
    assert!(app.editor.document().has_styles());
    app
}
#[test]
fn styled_png_consent_is_fresh_unchecked_and_cancel_keeps_original() {
    let mut app = styled();
    let source = app.editor.document_snapshot();
    let history = app.editor.history_view();
    assert_eq!(app.enabled_export_formats(), vec![ImageExport::Png]);
    app.begin_export_options(ImageExport::Png);
    let Some(Confirm::ExportOptions(draft)) = &app.confirm else {
        panic!("real export modal");
    };
    assert!(!draft.bake_appearance);
    assert!(!draft.bake_filters);
    assert_eq!(draft.frozen.appearance_counts, [1, 0, 0, 0, 0]);
    assert_eq!(
        draft.frozen.appearance_bake,
        export::appearance_bake::Policy::Refuse
    );
    let mut h = Harness::builder()
        .with_size(egui::vec2(900., 750.))
        .build_ui_state(|ui, app: &mut App| app.confirm_dialog(ui), app);
    h.run();
    h.get_by_label("Bake editable layer appearance into this PNG copy")
        .click();
    h.run();
    let Some(Confirm::ExportOptions(draft)) = &h.state().confirm else {
        panic!("modal retained");
    };
    assert!(draft.bake_appearance);
    h.get_by_label("Cancel").click();
    h.run();
    assert!(Arc::ptr_eq(&source, &h.state().editor.document_snapshot()));
    assert_eq!(h.state().editor.history_view(), history);
    h.state_mut().begin_export_options(ImageExport::Png);
    h.run();
    let Some(Confirm::ExportOptions(draft)) = &h.state().confirm else {
        panic!("new modal");
    };
    assert!(!draft.bake_appearance);
    assert_eq!(
        draft.frozen.appearance_bake,
        export::appearance_bake::Policy::Refuse
    );
}
#[test]
fn same_frozen_styled_source_png_requires_witness_and_keeps_authoritative_bytes() {
    let app = styled();
    let frozen = Frozen::capture(&app).unwrap();
    let source = frozen.document.clone();
    let target = Target::new(
        paintcraft_formats::registry::Family::Png,
        paintcraft_formats::registry::Operation::ExportFlat,
    );
    let refused = export::appearance_bake::Policy::capture(
        &source,
        target,
        export::appearance_bake::Intent::Refuse,
        &|| Ok(()),
    )
    .unwrap();
    assert_eq!(refused, export::appearance_bake::Policy::Refuse);
    assert!(refused.validate(&source, target, &|| Ok(())).is_err());
    let witness = export::appearance_bake::Policy::capture(
        &source,
        target,
        export::appearance_bake::Intent::Bake,
        &|| Ok(()),
    )
    .unwrap();
    assert!(witness.validate(&source, target, &|| Ok(())).unwrap());
    let mut options = export::FlatOptions::new(ImageFormat::Png, 90);
    options.appearance_bake = witness;
    let allowed = KNOWN_LOSSES
        .iter()
        .map(|s| (*s).to_owned())
        .collect::<Vec<_>>();
    let plan = spatial_workspace::JobPolicy::capture(&source, None, false)
        .run(|| export::export_flat_options(&source, options, &allowed))
        .unwrap();
    assert_eq!(
        paintcraft_formats::image::decode(&plan.bytes).unwrap().rgba,
        [255, 0, 0, 255, 255, 0, 0, 255]
    );
    assert!(plan.losses.iter().any(|l| l == "styles-baked"));
    assert!(Arc::ptr_eq(&source, &app.editor.document_snapshot()));
    assert_eq!(
        source.layers[&source.roots[0]].raster.get(0, 0),
        Rgba([10, 20, 30, 255])
    );
}
