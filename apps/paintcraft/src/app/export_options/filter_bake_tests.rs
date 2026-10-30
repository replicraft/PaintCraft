//! Actual Studio options bind bake authority to one private copy.
use super::*;
use egui_kittest::{
    Harness,
    kittest::{NodeT, Queryable},
};
use paintcraft_core::types::filters::Descriptor;
use paintcraft_document::{FilterInstance, FilterStack};
const CHECK: &str = "Bake editable filters into this exported copy";
fn app(enabled: bool) -> App {
    let mut app = App::new(None);
    app.settings.experience = Experience::Studio;
    let mut doc = paintcraft_document::new_document(
        64,
        32,
        Some(Rgba([17, 31, 47, 255])),
        0,
        paintcraft_document::POLICY,
    )
    .unwrap();
    doc.layers.get_mut(&doc.roots[0]).unwrap().filters =
        FilterStack::new(vec![FilterInstance::new(
            Uuid::from_u128(1),
            enabled,
            Descriptor::InvertRgbV1,
        )])
        .unwrap();
    app.editor = Editor::with_document(doc, None);
    app
}
fn harness(app: App) -> Harness<'static, App> {
    Harness::builder()
        .with_size(egui::vec2(1100., 1100.))
        .build_ui_state(|ui, app: &mut App| app.confirm_dialog(ui), app)
}
#[test]
fn unchecked_even_disabled_stack_refuses_and_cancel_keeps_source_history() {
    for enabled in [false, true] {
        let mut app = app(enabled);
        let original = app.editor.document_snapshot();
        let history = app.editor.history_view().ticket;
        app.begin_export_options(ImageExport::Png);
        let mut h = harness(app);
        h.run();
        assert!(
            h.get_by_label("Choose destination…")
                .accesskit_node()
                .is_disabled()
        );
        h.get_by_label(CHECK).click();
        h.run();
        assert!(
            !h.get_by_label("Choose destination…")
                .accesskit_node()
                .is_disabled()
        );
        h.get_by_label(CHECK).click();
        h.run();
        assert!(
            h.get_by_label("Choose destination…")
                .accesskit_node()
                .is_disabled()
        );
        h.get_by_label("Cancel").click();
        h.run();
        assert!(h.state().confirm.is_none());
        assert!(Arc::ptr_eq(
            &original,
            &h.state().editor.document_snapshot()
        ));
        assert_eq!(history, h.state().editor.history_view().ticket);
    }
}
#[test]
fn checked_copy_exports_filtered_pixels_once_and_original_witness_rejects_retarget() {
    let path = std::env::temp_dir().join(format!("paintcraft-filter-copy-{}.png", Uuid::new_v4()));
    let mut app = app(true);
    app.path_text = path.to_string_lossy().into_owned();
    let original = app.editor.document_snapshot();
    let history = app.editor.history_view().ticket;
    app.begin_export_options(ImageExport::Png);
    let mut h = harness(app);
    h.run();
    h.get_by_label(CHECK).click();
    h.run();
    h.get_by_label("Choose destination…").click();
    h.run();
    let Some(Confirm::Export { frozen, losses, .. }) = &h.state().confirm else {
        panic!("review")
    };
    assert!(
        losses
            .iter()
            .any(|s| s == export::filter_bake::BAKED_REASON)
    );
    assert!(
        frozen
            .filter_bake
            .validate(
                &frozen.document,
                Target::new(
                    paintcraft_formats::registry::Family::Bmp,
                    paintcraft_formats::registry::Operation::ExportFlat
                ),
                &|| Ok(())
            )
            .is_err()
    );
    h.get_by_label("Export now").click();
    h.run();
    let rgba = paintcraft_formats::image::decode(&std::fs::read(&path).unwrap())
        .unwrap()
        .rgba;
    assert_eq!(rgba, [238, 224, 208, 255].repeat(64 * 32));
    assert!(Arc::ptr_eq(
        &original,
        &h.state().editor.document_snapshot()
    ));
    assert_eq!(history, h.state().editor.history_view().ticket);
    std::fs::remove_file(path).unwrap();
}
#[test]
fn retained_private_text_uses_once_frozen_filtered_copy_without_acknowledging_draft() {
    let mut app = app(true);
    app.editor.select_tool(Tool::Text);
    app.editor.pointer_down((4, 4));
    app.editor.pointer_up();
    app.editor.type_text("Ink");
    let original = app.editor.document_snapshot();
    let history = app.editor.history_view().ticket;
    app.begin_export_options(ImageExport::Png);
    let Some(Confirm::ExportOptions(draft)) = &app.confirm else {
        panic!("options")
    };
    let frozen = draft.frozen.clone();
    assert_ne!(frozen.document.state_id, original.state_id);
    let target = Target::new(
        paintcraft_formats::registry::Family::Png,
        paintcraft_formats::registry::Operation::ExportFlat,
    );
    let policy = export::filter_bake::Policy::capture(
        &frozen.document,
        target,
        export::filter_bake::Intent::Bake,
        &|| Ok(()),
    )
    .unwrap();
    assert!(policy.validate(&original, target, &|| Ok(())).is_err());
    let mut options = export::FlatOptions::new(ImageFormat::Png, 90);
    options.filter_bake = policy;
    let copy =
        export::export_flat_options(&frozen.document, options, &KNOWN_LOSSES.map(str::to_owned))
            .unwrap();
    assert_eq!(
        paintcraft_formats::image::decode(&copy.bytes).unwrap().rgba,
        paintcraft_render::render_flat(&frozen.document).unwrap()
    );
    assert!(app.editor.is_editing_text());
    assert!(Arc::ptr_eq(&original, &app.editor.document_snapshot()));
    assert_eq!(history, app.editor.history_view().ticket);
}

#[test]
fn advanced_dds_msp_require_their_own_bake_choice_and_revocation_retires_preview() {
    use std::time::{Duration, Instant};
    for format in [ImageExport::Dds, ImageExport::Msp] {
        let mut app = app(true);
        let mut doc = (*app.editor.document_snapshot()).clone();
        doc.layers.get_mut(&doc.roots[0]).unwrap().raster =
            paintcraft_raster::RasterLayer::new(64, 32, Rgba([0, 0, 0, 255]));
        app.editor = Editor::with_document(doc, None);
        app.initialize_editing(app.context.clone());
        app.begin_export_options(format);
        let Some(Confirm::ExportOptions(draft)) = &mut app.confirm else {
            panic!("options")
        };
        if format == ImageExport::Dds {
            draft.dds.mips = paintcraft_ui::dds_export::Mips::Full;
        }
        let original = app.editor.document_snapshot();
        let history = app.editor.history_view().ticket;
        let mut h = Harness::builder()
            .with_max_steps(40)
            .with_size(egui::vec2(1100., 1100.))
            .build_ui_state(
                |ui, app: &mut App| {
                    app.poll_edits(ui.ctx());
                    app.poll_dds_export(ui.ctx());
                    app.poll_msp_export(ui.ctx());
                    app.confirm_dialog(ui);
                },
                app,
            );
        h.run();
        assert!(
            h.query_by_label(CHECK).is_none(),
            "handoff does not ask twice"
        );
        h.get_by_label("Preview export").click();
        h.run();
        assert!(
            h.get_by_label("Choose destination…")
                .accesskit_node()
                .is_disabled()
        );
        h.get_by_label(CHECK).click();
        h.step();
        let until = Instant::now() + Duration::from_secs(15);
        while h
            .get_by_label("Choose destination…")
            .accesskit_node()
            .is_disabled()
        {
            h.step();
            assert!(Instant::now() < until, "{}", h.state().editor.status());
            std::thread::sleep(Duration::from_millis(1));
        }
        h.run();
        let preview_label = if format == ImageExport::Dds {
            "Base image preview"
        } else {
            "Export preview"
        };
        assert!(h.query_by_label(preview_label).is_some());
        h.get_by_label(CHECK).click();
        h.run();
        assert!(
            h.get_by_label("Choose destination…")
                .accesskit_node()
                .is_disabled()
        );
        assert!(h.query_by_label(preview_label).is_none());
        h.get_by_label("Cancel").click();
        h.run();
        assert!(Arc::ptr_eq(
            &original,
            &h.state().editor.document_snapshot()
        ));
        assert_eq!(history, h.state().editor.history_view().ticket);
        let path = std::env::temp_dir().join(format!(
            "paintcraft-baked-{}.{}",
            Uuid::new_v4(),
            format.extension()
        ));
        h.state_mut().path_text = path.with_extension("pcnd").to_string_lossy().into_owned();
        h.state_mut().begin_export_options(format);
        let Some(Confirm::ExportOptions(draft)) = &mut h.state_mut().confirm else {
            panic!("options")
        };
        if format == ImageExport::Dds {
            draft.dds.mips = paintcraft_ui::dds_export::Mips::Full;
        }
        h.run();
        h.get_by_label("Preview export").click();
        h.run();
        h.get_by_label(CHECK).click();
        h.step();
        let until = Instant::now() + Duration::from_secs(15);
        while h
            .get_by_label("Choose destination…")
            .accesskit_node()
            .is_disabled()
        {
            h.step();
            assert!(Instant::now() < until, "{}", h.state().editor.status());
            std::thread::sleep(Duration::from_millis(1));
        }
        h.run();
        h.get_by_label("Choose destination…").click();
        h.run();
        h.get_by_label("Export now").click();
        h.run();
        let bytes = std::fs::read(&path).unwrap();
        let rgba = if format == ImageExport::Dds {
            paintcraft_formats::dds::decode_top_mip_checked(&bytes, 64 * 32, &|| Ok(()))
                .unwrap()
                .rgba
        } else {
            paintcraft_formats::msp::decode(&bytes, &|| Ok(()))
                .unwrap()
                .rgba(&|| Ok(()))
                .unwrap()
        };
        assert_eq!(rgba, [255u8; 4].repeat(64 * 32));
        std::fs::remove_file(path).unwrap();
        assert!(Arc::ptr_eq(
            &original,
            &h.state().editor.document_snapshot()
        ));
        assert_eq!(history, h.state().editor.history_view().ticket);
    }
}
