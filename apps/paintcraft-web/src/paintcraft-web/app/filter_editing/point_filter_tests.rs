//! Shared controls + real editor/CPU preview; Worker readiness is host-simulated.
use super::*;
use egui_kittest::{Harness, kittest::Queryable};
use paintcraft_core::types::Rect;
use paintcraft_document::{FilterInstance, FilterMask, FilterStack};
use paintcraft_raster::coverage_mask::{A8Mask, Limits};
use paintcraft_studio::filters::Kind;
use std::sync::Arc;
const RAW: [u8; 8] = [10, 20, 30, 255, 100, 150, 200, 255];
const CROSS: [u8; 8] = [20, 115, 0, 255, 0, 178, 4, 255];
fn setup(raw: &[u8; 8]) -> WebApp {
    let ctx = egui::Context::default();
    let mut app = WebApp::new(&ctx);
    app.frontend = Frontend::Studio;
    app.editor = Editor::from_document(
        paintcraft_document::import_image(2, 1, raw, "web-point-ui/v1").unwrap(),
    )
    .unwrap();
    app.canvas.filter_view = Some(
        View::new(
            app.editor.session.document(),
            Rect::new(0, 0, 2, 1).unwrap(),
            0,
        )
        .unwrap(),
    );
    app
}
fn harness(app: WebApp) -> Harness<'static, WebApp> {
    Harness::builder()
        .with_size(egui::vec2(1000., 800.))
        .build_ui_state(
            |ui, app: &mut WebApp| {
                app.studio_filter_dialog(ui.ctx());
            },
            app,
        )
}
fn request(app: &WebApp) -> Request {
    let d = &app.filter_editing.as_ref().unwrap().draft;
    Request {
        revision: d.revision,
        edit: Some(d.operation().unwrap()),
        render: d.parameters.preview,
        view: app.canvas.filter_view.unwrap(),
    }
}
fn preview(app: &WebApp) -> paintcraft_document::Document {
    app.editor
        .display_preview(Some(paintcraft_tools::Preview::Filter(
            request(app).edit.unwrap(),
        )))
        .unwrap()
        .unwrap()
}
fn admit(app: &mut WebApp, ctx: &egui::Context) {
    let request = request(app);
    app.filter_display.renderer.accept_test(request.clone());
    app.accept_filter_reply(
        ctx,
        render::Reply {
            request,
            size: (2, 1),
            pixels: None,
        },
    );
}
#[test]
fn four_add_controls_keep_private_pixels_cancel_then_publish_one_undo() {
    for (kind, expected) in [
        (Kind::BlackWhite, [19, 19, 19, 255, 143, 143, 143, 255]),
        (Kind::Threshold, [0, 0, 0, 255, 255, 255, 255, 255]),
        (Kind::Posterize, [0, 0, 51, 255, 102, 153, 204, 255]),
        (Kind::Solarize, [10, 20, 30, 255, 100, 105, 55, 255]),
    ] {
        let mut h = harness(setup(&RAW));
        h.run();
        let ctx = h.ctx.clone();
        let source = h.state().editor.session.snapshot();
        h.state_mut().begin_studio_filter(&ctx, Intent::Add(kind));
        h.run();
        assert_eq!(
            paintcraft_render::render_flat(&preview(h.state())).unwrap(),
            expected
        );
        assert!(Arc::ptr_eq(&source, &h.state().editor.session.snapshot()));
        assert_eq!(h.state().editor.session.undo_depth(), 0);
        h.get_by_label("Cancel").click();
        h.run();
        assert!(h.state().filter_editing.is_none());
        assert!(Arc::ptr_eq(&source, &h.state().editor.session.snapshot()));
        h.state_mut().begin_studio_filter(&ctx, Intent::Add(kind));
        h.run();
        admit(h.state_mut(), &ctx);
        h.get_by_label("Apply").click();
        h.run();
        assert!(h.state().filter_editing.is_none());
        assert_eq!(
            paintcraft_render::render_flat(h.state().editor.session.document()).unwrap(),
            expected
        );
        assert_eq!(h.state().editor.session.undo_depth(), 1);
        let doc = h.state().editor.session.document();
        let layer = &doc.layers[&doc.roots[0]];
        assert_eq!(
            layer.filters.as_slice()[0].descriptor(),
            Parameters::new(kind).descriptor().unwrap()
        );
        assert_eq!(layer.raster, source.layers[&source.roots[0]].raster);
        h.state_mut().editor.apply(Edit::Undo).unwrap();
        assert_eq!(
            paintcraft_render::render_flat(h.state().editor.session.document()).unwrap(),
            RAW
        );
    }
}
fn masked_app(kind: Kind) -> WebApp {
    let mut app = setup(&CROSS);
    let mut doc = (*app.editor.session.snapshot()).clone();
    let mut params = Parameters::new(kind);
    if kind == Kind::BlackWhite {
        params.strength = 0;
    }
    let coverage = A8Mask::uniform(
        Rect::new(0, 0, 2, 1).unwrap(),
        128,
        &paintcraft_core::types::resources::default_coverage_budget(),
        Limits::default(),
        &|| Ok(()),
    )
    .unwrap();
    doc.layers.get_mut(&doc.roots[0]).unwrap().filters = FilterStack::new(vec![
        FilterInstance::new(Uuid::from_u128(82), true, params.descriptor().unwrap())
            .with_mask(Some(FilterMask::new(true, coverage))),
    ])
    .unwrap();
    app.editor = Editor::from_document(doc).unwrap();
    app
}
#[test]
fn four_edit_controls_preserve_fractional_masks_and_off_preview_final_parameters() {
    for (kind, control, expected) in [
        (
            Kind::BlackWhite,
            Some(("Strength (%)", 100.)),
            [54, 101, 44, 255, 64, 153, 66, 255],
        ),
        (
            Kind::Threshold,
            Some(("Threshold", 127.)),
            [10, 57, 0, 255, 128, 217, 130, 255],
        ),
        (
            Kind::Posterize,
            Some(("Levels", 3.)),
            [10, 122, 0, 255, 0, 153, 2, 255],
        ),
        (Kind::Solarize, None, [20, 115, 0, 255, 0, 127, 4, 255]),
    ] {
        let mut h = harness(masked_app(kind));
        h.run();
        let ctx = h.ctx.clone();
        let source = h.state().editor.session.snapshot();
        let old_mask = source.layers[&source.roots[0]].filters.as_slice()[0]
            .mask()
            .unwrap()
            .clone();
        h.state_mut()
            .begin_studio_filter(&ctx, Intent::Edit(Uuid::from_u128(82)));
        h.run();
        if let Some((label, value)) = control {
            h.get_by_role_and_label(egui::Role::Slider, label)
                .set_value_accesskit(value);
            h.run();
        }
        let candidate = preview(h.state());
        assert_eq!(
            paintcraft_render::render_flat(&candidate).unwrap(),
            expected,
            "{kind:?}"
        );
        assert_eq!(
            candidate.layers[&candidate.roots[0]].filters.as_slice()[0].mask(),
            Some(&old_mask)
        );
        assert!(Arc::ptr_eq(&source, &h.state().editor.session.snapshot()));
        h.get_by_label("Preview").click();
        h.run();
        assert!(
            !h.state()
                .filter_editing
                .as_ref()
                .unwrap()
                .draft
                .parameters
                .preview
        );
        admit(h.state_mut(), &ctx);
        h.get_by_label("Apply").click();
        h.run();
        assert!(h.state().filter_editing.is_none());
        let doc = h.state().editor.session.document();
        let layer = &doc.layers[&doc.roots[0]];
        assert_eq!(layer.filters.as_slice()[0].mask(), Some(&old_mask));
        assert_eq!(layer.raster, source.layers[&source.roots[0]].raster);
        assert_eq!(paintcraft_render::render_flat(doc).unwrap(), expected);
        assert_eq!(
            h.state().editor.session.undo_depth(),
            usize::from(kind != Kind::Solarize)
        );
        if kind != Kind::Solarize {
            h.state_mut().editor.apply(Edit::Undo).unwrap();
            assert!(h.state().editor.session.document().same_semantics(&source));
        }
    }
}
