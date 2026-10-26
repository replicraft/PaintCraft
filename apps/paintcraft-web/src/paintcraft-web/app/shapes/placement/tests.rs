use super::*;
use egui_kittest::{
    Harness,
    kittest::{NodeT, Queryable},
};
use paintcraft_ui::i18n::{self, Language, translate};
const SOURCE: &[u8] = include_bytes!(
    "../../../../../../../crates/services/paintcraft-formats/tests/csh-fixtures/placement.csh"
);
fn setup(language: Language) -> Harness<'static, WebApp> {
    let ctx = egui::Context::default();
    let mut app = WebApp::new(&ctx);
    app.frontend = Frontend::Studio;
    app.editor = Editor::from_document(
        paintcraft_document::new_document(8, 8, None, 0, paintcraft_document::POLICY).unwrap(),
    )
    .unwrap();
    app.begin_shapes();
    app.accept_shape(
        &ctx,
        app.shapes.epoch,
        Loaded::new(SOURCE, "placement.csh".into()).map(Some),
    );
    Harness::builder()
        .with_max_steps(30)
        .with_size(egui::vec2(1100.0, 900.0))
        .build_ui_state(
            move |ui, app: &mut WebApp| {
                if ui.ctx().missing_glyph_policy() != egui::epaint::text::MissingGlyphPolicy::Tofu {
                    paintcraft_ui::fonts::install(ui.ctx());
                    ui.ctx()
                        .set_missing_glyph_policy(egui::epaint::text::MissingGlyphPolicy::Tofu);
                }
                i18n::set_language(ui.ctx(), language);
                app.dialogs(ui.ctx());
            },
            app,
        )
}
fn preview(h: &mut Harness<WebApp>, lang: Language) {
    h.get_by_role_and_label(egui::Role::Button, &translate(lang, "Update preview"))
        .click();
    h.run();
}
fn approve(h: &mut Harness<WebApp>, lang: Language) {
    h.get_by_role_and_label(
        egui::Role::CheckBox,
        &translate(lang, "I accept converting this shape to pixels."),
    )
    .scroll_to_me();
    h.run();
    h.get_by_role_and_label(
        egui::Role::CheckBox,
        &translate(lang, "I accept converting this shape to pixels."),
    )
    .click();
    h.run();
}
#[test]
fn browser_numeric_shape_placement_matches_exact_native_signed_pixels_in_english_and_japanese() {
    for lang in [Language::English, Language::Japanese] {
        let mut h = setup(lang);
        h.run();
        let before = h.state().editor.session.snapshot();
        let name = h.state().name.clone();
        h.get_by_role_and_label(egui::Role::Button, &translate(lang, "Place shape…"))
            .click();
        h.run();
        h.state_mut()
            .shapes
            .placement
            .as_mut()
            .unwrap()
            .state
            .config = Config {
            at: (-2, 1),
            width: 4,
            height: 3,
            color: paintcraft_core::types::Rgba([200, 40, 10, 128]),
        };
        h.run();
        preview(&mut h, lang);
        assert!(Arc::ptr_eq(&before, &h.state().editor.session.snapshot()));
        assert!(!h.state().editor.session.is_dirty());
        approve(&mut h, lang);
        h.get_by_role_and_label(egui::Role::Button, &translate(lang, "Place shape"))
            .click();
        h.run();
        assert_eq!(h.state().editor.session.undo_depth(), 1);
        assert_eq!(
            h.state()
                .editor
                .session
                .history_view()
                .entries
                .last()
                .unwrap()
                .label
                .text(),
            "Shape"
        );
        assert_eq!(h.state().name, name);
        let layer = h.state().editor.session.document().roots[0];
        for y in 0..8 {
            for x in 0..8 {
                assert_eq!(
                    h.state().editor.session.document().layers[&layer]
                        .raster
                        .get(x, y),
                    if x < 2 && (1..4).contains(&y) {
                        paintcraft_core::types::Rgba([200, 40, 10, 128])
                    } else {
                        paintcraft_core::types::Rgba::TRANSPARENT
                    }
                );
            }
        }
        h.state_mut()
            .editor
            .apply(paintcraft_tools::Action::Undo)
            .unwrap();
        assert!(h.state().editor.session.document().layers.is_empty());
        h.state_mut()
            .editor
            .apply(paintcraft_tools::Action::Redo)
            .unwrap();
        assert_eq!(h.state().editor.session.document().layers.len(), 1);
    }
}
#[test]
fn browser_stale_resource_invalid_options_cancellation_and_noop_keep_history_atomic() {
    let mut h = setup(Language::English);
    h.run();
    let before = h.state().editor.session.snapshot();
    h.state_mut().begin_shape_placement().unwrap();
    h.run();
    let budget = h.state().shapes.placement.as_ref().unwrap().budget.clone();
    h.state_mut()
        .shapes
        .placement
        .as_mut()
        .unwrap()
        .state
        .config
        .width = 0;
    h.run();
    assert!(
        h.get_by_role_and_label(egui::Role::Button, "Update preview")
            .accesskit_node()
            .is_disabled()
    );
    h.state_mut()
        .shapes
        .placement
        .as_mut()
        .unwrap()
        .state
        .config
        .width = 4;
    h.run();
    preview(&mut h, Language::English);
    assert!(budget.usage().reserved > 0);
    h.state_mut().shapes.loaded = Some(Arc::new(
        Loaded::new(SOURCE, "replacement.csh".into()).unwrap(),
    ));
    h.run();
    assert!(h.state().shapes.placement.is_none());
    assert_eq!(budget.usage().reserved, 0);
    assert!(Arc::ptr_eq(&before, &h.state().editor.session.snapshot()));
    h.state_mut().begin_shape_placement().unwrap();
    h.run();
    preview(&mut h, Language::English);
    h.get_by_role_and_label(egui::Role::Button, "Cancel")
        .click();
    h.run();
    assert!(Arc::ptr_eq(&before, &h.state().editor.session.snapshot()));
    h.state_mut().begin_shape_placement().unwrap();
    h.state_mut()
        .shapes
        .placement
        .as_mut()
        .unwrap()
        .state
        .config
        .color = paintcraft_core::types::Rgba::TRANSPARENT;
    h.run();
    preview(&mut h, Language::English);
    approve(&mut h, Language::English);
    h.get_by_role_and_label(egui::Role::Button, "Place shape")
        .click();
    h.run();
    assert!(Arc::ptr_eq(&before, &h.state().editor.session.snapshot()));
    assert_eq!(h.state().editor.session.undo_depth(), 0);
    assert!(h.state().editor.session.document().layers.is_empty());
}
fn adopt_drag(h: &mut Harness<WebApp>, lang: Language) {
    h.get_by_role_and_label(egui::Role::Button, &translate(lang, "Use as shape tool…"))
        .scroll_to_me();
    h.run();
    h.get_by_role_and_label(egui::Role::Button, &translate(lang, "Use as shape tool…"))
        .click();
    h.run();
    preview(h, lang);
    h.get_by_role_and_label(egui::Role::Button, &translate(lang, "Use as shape tool"))
        .click();
    h.run();
    assert!(h.state().editor.resource_shape_name().is_none());
    h.get_by_role_and_label(
        egui::Role::CheckBox,
        &translate(lang, "I accept converting each drawn shape to pixels."),
    )
    .scroll_to_me();
    h.run();
    h.get_by_role_and_label(
        egui::Role::CheckBox,
        &translate(lang, "I accept converting each drawn shape to pixels."),
    )
    .click();
    h.run();
    h.get_by_role_and_label(egui::Role::Button, &translate(lang, "Use as shape tool"))
        .click();
    h.run();
    assert!(h.state().editor.resource_shape_name().is_some());
}
#[test]
fn browser_drag_adoption_matches_native_pixels_and_retains_original_resource_after_replacement() {
    use paintcraft_core::types::Rgba;
    for lang in [Language::English, Language::Japanese] {
        let mut h = setup(lang);
        h.run();
        let before = h.state().editor.session.snapshot();
        let name = h.state().name.clone();
        adopt_drag(&mut h, lang);
        assert!(Arc::ptr_eq(&before, &h.state().editor.session.snapshot()));
        assert!(!h.state().editor.session.is_dirty());
        let old = h.state().editor.resource_shape_name().unwrap().to_owned();
        h.state_mut().begin_shapes();
        let ctx = h.ctx.clone();
        let epoch = h.state().shapes.epoch;
        h.state_mut().accept_shape(
            &ctx,
            epoch,
            Loaded::new(SOURCE, "replacement.csh".into()).map(Some),
        );
        assert_eq!(h.state().editor.resource_shape_name(), Some(old.as_str()));
        let e = &mut h.state_mut().editor;
        e.tools.color = Rgba([200, 40, 10, 128]);
        e.tools.pointer_down(uuid::Uuid::nil(), (1, 3));
        e.tools.pointer_move((-2, 1));
        let preview = e.display_document().unwrap().unwrap();
        let pixels = paintcraft_render::render_flat(&preview).unwrap();
        let (_, action) = e.tools.pointer_up().unwrap();
        e.apply(action).unwrap();
        assert_eq!(
            paintcraft_render::render_flat(e.session.document()).unwrap(),
            pixels
        );
        assert_eq!(e.session.undo_depth(), 1);
        assert_eq!(
            e.session
                .history_view()
                .entries
                .last()
                .unwrap()
                .label
                .text(),
            "Shape"
        );
        let layer = e.session.document().roots[0];
        for y in 0..8 {
            for x in 0..8 {
                assert_eq!(
                    e.session.document().layers[&layer].raster.get(x, y),
                    if x < 2 && (1..4).contains(&y) {
                        Rgba([200, 40, 10, 128])
                    } else {
                        Rgba::TRANSPARENT
                    }
                );
            }
        }
        e.apply(paintcraft_tools::Action::Undo).unwrap();
        assert!(e.session.document().layers.is_empty());
        e.apply(paintcraft_tools::Action::Redo).unwrap();
        assert_eq!(
            paintcraft_render::render_flat(e.session.document()).unwrap(),
            pixels
        );
        assert_eq!(h.state().name, name);
    }
}
#[test]
fn browser_cancelled_stale_review_and_classic_workspace_preserve_separate_shape_ownership() {
    let mut h = setup(Language::English);
    h.run();
    adopt_drag(&mut h, Language::English);
    let before = h.state().editor.session.snapshot();
    let old = h.state().editor.resource_shape_name().unwrap().to_owned();
    h.state_mut().begin_shapes();
    h.state_mut().shapes.selected = 1;
    h.state_mut().begin_shape_tool().unwrap();
    h.run();
    preview(&mut h, Language::English);
    h.get_by_role_and_label(egui::Role::Button, "Cancel")
        .click();
    h.run();
    assert!(h.state().shapes.placement.is_none());
    assert_eq!(h.state().editor.resource_shape_name(), Some(old.as_str()));
    h.state_mut().begin_shape_tool().unwrap();
    h.run();
    preview(&mut h, Language::English);
    h.state_mut().shapes.selected = 2;
    h.run();
    assert!(h.state().shapes.placement.is_none());
    assert!(h.state_mut().begin_shape_tool().is_err());
    assert!(Arc::ptr_eq(&before, &h.state().editor.session.snapshot()));
    let ctx = h.ctx.clone();
    h.state_mut().switch_frontend(&ctx, Frontend::Classic);
    assert!(h.state().editor.resource_shape_name().is_none());
    assert!(h.state().editor.tools.resource_shape.is_none());
    h.state_mut().switch_frontend(&ctx, Frontend::Studio);
    assert_eq!(h.state().editor.resource_shape_name(), Some(old.as_str()));
    assert!(Arc::ptr_eq(&before, &h.state().editor.session.snapshot()));
}
