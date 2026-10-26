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
