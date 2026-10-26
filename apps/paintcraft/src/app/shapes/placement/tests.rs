use super::*;
use egui_kittest::{Harness, kittest::Queryable};
use paintcraft_ui::i18n::{self, Language, translate};
const SOURCE: &[u8] = include_bytes!(
    "../../../../../../crates/services/paintcraft-formats/tests/csh-fixtures/placement.csh"
);
fn setup(
    language: Language,
    background: Option<Rgba>,
) -> (Harness<'static, App>, PathBuf, PathBuf) {
    let root = std::env::temp_dir().join(format!("paintcraft-shape-place-{}", Uuid::new_v4()));
    std::fs::create_dir(&root).unwrap();
    let source = root.join("placement.csh");
    std::fs::write(&source, SOURCE).unwrap();
    let mut app = App::new(None);
    app.settings.experience = Experience::Studio;
    app.editor = Editor::with_document(
        paintcraft_document::new_document(8, 8, background, 0, paintcraft_document::POLICY)
            .unwrap(),
        None,
    );
    app.begin_shapes();
    app.install_shape(source.clone()).unwrap();
    let harness = Harness::builder()
        .with_max_steps(30)
        .with_size(egui::vec2(1100.0, 900.0))
        .build_ui_state(
            move |ui, app: &mut App| {
                if ui.ctx().missing_glyph_policy() != egui::epaint::text::MissingGlyphPolicy::Tofu {
                    paintcraft_ui::fonts::install(ui.ctx());
                    ui.ctx()
                        .set_missing_glyph_policy(egui::epaint::text::MissingGlyphPolicy::Tofu);
                }
                i18n::set_language(ui.ctx(), language);
                app.confirm_dialog(ui);
            },
            app,
        );
    (harness, root, source)
}
fn preview(h: &mut Harness<App>, language: Language) {
    h.get_by_role_and_label(egui::Role::Button, &translate(language, "Update preview"))
        .click();
    h.run();
}
fn approve(h: &mut Harness<App>, language: Language) {
    h.get_by_role_and_label(
        egui::Role::CheckBox,
        &translate(language, "I accept converting this shape to pixels."),
    )
    .scroll_to_me();
    h.run();
    h.get_by_role_and_label(
        egui::Role::CheckBox,
        &translate(language, "I accept converting this shape to pixels."),
    )
    .click();
    h.run();
}
fn place(h: &mut Harness<App>, language: Language) {
    h.get_by_role_and_label(egui::Role::Button, &translate(language, "Place shape"))
        .click();
    h.run();
    h.state_mut().editor.run_pending();
    h.run();
}
#[test]
fn translated_numeric_shape_preview_consent_and_placement_are_atomic_with_exact_signed_pixels() {
    for language in [Language::English, Language::Japanese] {
        let (mut h, root, _) = setup(language, None);
        h.run();
        let before = h.state().editor.document_snapshot();
        h.get_by_role_and_label(egui::Role::Button, &translate(language, "Place shape…"))
            .click();
        h.run();
        let budget = h.state().shapes.placement.as_ref().unwrap().budget.clone();
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
            color: Rgba([200, 40, 10, 128]),
        };
        h.run();
        preview(&mut h, language);
        assert!(Arc::ptr_eq(&before, &h.state().editor.document_snapshot()));
        assert!(budget.usage().reserved > 0);
        place(&mut h, language);
        assert!(
            Arc::ptr_eq(&before, &h.state().editor.document_snapshot()),
            "unapproved placement must not publish"
        );
        approve(&mut h, language);
        place(&mut h, language);
        assert_eq!(h.state().editor.undo_depth(), 1);
        assert_eq!(
            h.state()
                .editor
                .history_view()
                .entries
                .last()
                .unwrap()
                .label
                .text(),
            "Shape"
        );
        let layer = h.state().editor.document().roots[0];
        for y in 0..8 {
            for x in 0..8 {
                assert_eq!(
                    h.state().editor.document().layers[&layer].raster.get(x, y),
                    if x < 2 && (1..4).contains(&y) {
                        Rgba([200, 40, 10, 128])
                    } else {
                        Rgba::TRANSPARENT
                    }
                );
            }
        }
        assert_eq!(budget.usage().reserved, 0);
        h.state_mut().editor.request(900, Action::Undo);
        h.state_mut().editor.run_pending();
        assert!(h.state().editor.document().layers.is_empty());
        drop(h);
        std::fs::remove_dir_all(root).unwrap();
    }
}
#[test]
fn unsupported_hidden_stale_and_cancelled_shape_choices_keep_document_and_release_preview() {
    let (mut h, root, _) = setup(Language::English, Some(Rgba([255; 4])));
    h.run();
    let before = h.state().editor.document_snapshot();
    h.state_mut().shapes.selected = 2;
    assert!(h.state_mut().begin_shape_placement().is_err());
    assert!(h.state().shapes.placement.is_none());
    h.state_mut().shapes.selected = 0;
    let layer = h.state().editor.target().unwrap();
    h.state_mut().editor.request(
        901,
        Action::SetVisible {
            layer,
            visible: false,
        },
    );
    h.state_mut().editor.run_pending();
    assert!(h.state_mut().begin_shape_placement().is_err());
    h.state_mut().editor.request(902, Action::Undo);
    h.state_mut().editor.run_pending();
    h.state_mut().begin_shape_placement().unwrap();
    h.run();
    preview(&mut h, Language::English);
    let budget = h.state().shapes.placement.as_ref().unwrap().budget.clone();
    assert!(budget.usage().reserved > 0);
    h.get_by_role_and_label(egui::Role::Button, "Cancel")
        .click();
    h.run();
    assert_eq!(budget.usage().reserved, 0);
    assert!(h.state().shapes.placement.is_none());
    assert!(h.state().editor.document().same_semantics(&before));
    h.state_mut().begin_shape_placement().unwrap();
    h.run();
    preview(&mut h, Language::English);
    approve(&mut h, Language::English);
    let budget = h.state().shapes.placement.as_ref().unwrap().budget.clone();
    h.state_mut().shapes.selected = 1;
    h.run();
    assert!(h.state().shapes.placement.is_none());
    assert_eq!(budget.usage().reserved, 0);
    assert!(h.state().editor.document().same_semantics(&before));
    drop(h);
    std::fs::remove_dir_all(root).unwrap();
}
#[test]
fn pending_shape_source_identity_stays_protected_after_rename_and_library_replacement() {
    let (mut h, root, source) = setup(Language::English, None);
    h.run();
    h.state_mut().begin_shape_placement().unwrap();
    let before = h.state().editor.document_snapshot();
    let moved = root.join("original-moved.csh");
    std::fs::rename(&source, &moved).unwrap();
    let replacement = root.join("replacement.csh");
    std::fs::write(&replacement, SOURCE).unwrap();
    h.state_mut().install_shape(replacement).unwrap();
    let alias = root.join("pending.png");
    std::fs::hard_link(&moved, &alias).unwrap();
    h.state_mut()
        .do_export(&alias, ImageFormat::Png, &KNOWN_LOSSES.map(str::to_owned));
    assert_eq!(std::fs::read(&alias).unwrap(), SOURCE);
    let project = root.join("pending.pcnd");
    std::fs::hard_link(&moved, &project).unwrap();
    let ctx = h.ctx.clone();
    h.state_mut().save_selected(&ctx, project.clone());
    assert_eq!(std::fs::read(&project).unwrap(), SOURCE);
    assert!(Arc::ptr_eq(&before, &h.state().editor.document_snapshot()));
    h.run();
    assert!(h.state().shapes.placement.is_none());
    assert!(h.state().guard_export_destination(&alias).is_ok());
    drop(h);
    std::fs::remove_dir_all(root).unwrap();
}
#[test]
fn placed_shape_sources_follow_saved_documents_other_tabs_and_separate_workspaces() {
    let (mut h, root, source) = setup(Language::English, None);
    h.run();
    h.state_mut().begin_shape_placement().unwrap();
    h.run();
    preview(&mut h, Language::English);
    approve(&mut h, Language::English);
    place(&mut h, Language::English);
    assert_eq!(h.state().auxiliary_sources.len(), 1);
    let replacement = root.join("replacement.csh");
    std::fs::write(&replacement, SOURCE).unwrap();
    h.state_mut().install_shape(replacement).unwrap();
    let moved = root.join("original-moved.csh");
    std::fs::rename(&source, &moved).unwrap();
    let ctx = h.ctx.clone();
    let project = root.join("converted.pcnd");
    h.state_mut().save_selected(&ctx, project.clone());
    h.state_mut().editor.run_pending();
    h.state_mut().finish_save();
    assert!(project.is_file());
    for location in 0..3 {
        let app = h.state_mut();
        match location {
            1 => app
                .add_document_tab(Editor::new_document(2, 2).unwrap(), &ctx)
                .unwrap(),
            2 => {
                let mut parked = tabs::WorkspaceDocuments::new((2, 2), false);
                parked.swap(app);
                app.other_workspace = Some(Box::new(parked));
                app.settings.experience = Experience::Classic;
            }
            _ => {}
        }
        let alias = root.join(format!("placed-{location}.png"));
        std::fs::hard_link(&moved, &alias).unwrap();
        app.do_export(&alias, ImageFormat::Png, &KNOWN_LOSSES.map(str::to_owned));
        assert_eq!(std::fs::read(alias).unwrap(), SOURCE);
        let alias = root.join(format!("placed-{location}.pcnd"));
        std::fs::hard_link(&moved, &alias).unwrap();
        app.save_selected(&ctx, alias.clone());
        assert_eq!(std::fs::read(alias).unwrap(), SOURCE);
    }
    drop(h);
    std::fs::remove_dir_all(root).unwrap();
}
