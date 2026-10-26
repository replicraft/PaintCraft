use super::*;
use egui_kittest::{
    Harness,
    kittest::{NodeT, Queryable},
};
use paintcraft_ui::i18n::{self, Language, translate};
#[path = "../../../../../../../crates/services/paintcraft-formats/tests/support/svg_fonts.rs"]
mod fixture;

fn files() -> (PathBuf, PathBuf, PathBuf) {
    let root = std::env::temp_dir().join(format!("paintcraft-svg-fonts-{}", Uuid::new_v4()));
    std::fs::create_dir(&root).unwrap();
    let root = root.canonicalize().unwrap();
    let image = root.join("text.svg");
    let font = root.join("PaintCF.ttf");
    std::fs::write(&image, fixture::TEXT).unwrap();
    std::fs::write(&font, fixture::font()).unwrap();
    (root, image, font)
}
fn source(app: &mut App) -> &mut Source {
    let Candidate::Svg(source) = &mut app.pending_import.as_mut().unwrap().candidate else {
        panic!("expected SVG input");
    };
    source
}
fn pick(app: &mut App, ctx: &egui::Context, path: Option<PathBuf>) {
    let current = source(app);
    let picker = FontPick {
        source: current.id,
        files: current.font_files,
    };
    app.svg_font_picker = Some(picker);
    app.complete_dialog(
        ctx,
        paintcraft_platform::dialogs::Completion {
            kind: Kind::ImportSvgFont,
            path,
        },
    );
}
fn harness(path: &Path, language: Language) -> Harness<'static, App> {
    let mut app = App::new(None);
    app.settings.experience = Experience::Studio;
    app.import_selected(path.to_path_buf());
    source(&mut app).state.config.width = 128;
    source(&mut app).state.config.height = 48;
    Harness::builder()
        .with_max_steps(20)
        .with_size(Vec2::new(1360.0, 900.0))
        .build_ui_state(
            move |ui, app: &mut App| {
                i18n::set_language(ui.ctx(), language);
                app.import_dialog(ui.ctx());
            },
            app,
        )
}
fn preview(h: &mut Harness<'_, App>, language: Language) {
    h.get_by_role_and_label(egui::Role::Button, &translate(language, "Update preview"))
        .click();
    h.run();
}
fn approve(h: &mut Harness<'_, App>, language: Language) {
    h.get_by_label(&translate(language, "I accept these import changes."))
        .scroll_to_me();
    h.run();
    h.get_by_label(&translate(language, "I accept these import changes."))
        .click();
    h.run();
    assert!(
        source(h.state_mut()).state.accepted,
        "consent click did not take effect: {:?}",
        source(h.state_mut()).state
    );
    assert!(
        !h.get_by_role_and_label(egui::Role::Button, &translate(language, "Import image"))
            .accesskit_node()
            .is_disabled()
    );
    h.get_by_role_and_label(egui::Role::Button, &translate(language, "Import image"))
        .click();
    h.run();
}

#[test]
fn native_font_file_changes_real_svg_text_and_requires_fresh_preview_and_consent() {
    let (root, image, font) = files();
    for language in [Language::English, Language::Japanese] {
        let mut h = harness(&image, language);
        h.run();
        let before = h.state().editor.document_snapshot();
        preview(&mut h, language);
        let substitution = translate(
            language,
            paintcraft_ui::i18n::import_loss_description("svg-font-substituted").unwrap(),
        );
        assert!(h.query_by_label(&substitution).is_some());
        h.get_by_label(&translate(language, "I accept these import changes."))
            .scroll_to_me();
        h.run();
        h.get_by_label(&translate(language, "I accept these import changes."))
            .click();
        h.run();
        assert!(source(h.state_mut()).state.accepted);
        let ctx = h.ctx.clone();
        pick(h.state_mut(), &ctx, Some(font.clone()));
        h.run();
        assert!(!source(h.state_mut()).state.accepted);
        assert!(
            h.get_by_role_and_label(egui::Role::Button, &translate(language, "Import image"))
                .accesskit_node()
                .is_disabled()
        );
        assert!(Arc::ptr_eq(&before, &h.state().editor.document_snapshot()));
        preview(&mut h, language);
        assert!(h.query_by_label(&substitution).is_none());
        approve(&mut h, language);
        assert!(h.state().pending_import.is_none());
        assert_eq!(h.state().editor.document().canvas, (128, 48));
        assert_eq!(
            h.state().auxiliary_sources.len(),
            2,
            "main SVG and chosen font remain protected"
        );
        let bytes = fixture::font();
        let mut data = paintcraft_ui::fonts::bundled_data().to_vec();
        data.push(&bytes);
        let fonts = svg::Fonts::from_data(&data, "Selawik", &|| Ok(())).unwrap();
        let raster = svg::prepare(
            fixture::TEXT,
            svg::Options {
                width: 128,
                height: 48,
                dpi: 96.0,
                fit: svg::Fit::Contain,
            },
            &fonts,
            svg::PIXELS_MAX,
            &|| Ok(()),
        )
        .unwrap()
        .render(&|| Ok(()))
        .unwrap();
        assert_eq!(
            Sha256::digest(render_flat(h.state().editor.document()).unwrap()),
            Sha256::digest(&raster.rgba)
        );
    }
    std::fs::remove_dir_all(root).unwrap();
}

#[test]
fn failed_cancelled_and_stale_native_font_picks_keep_the_existing_preview_and_painting() {
    let (root, image, font) = files();
    let mut h = harness(&image, Language::English);
    h.run();
    let ctx = h.ctx.clone();
    pick(h.state_mut(), &ctx, Some(font));
    h.run();
    preview(&mut h, Language::English);
    h.get_by_label("I accept these import changes.")
        .scroll_to_me();
    h.run();
    h.get_by_label("I accept these import changes.").click();
    h.run();
    assert!(
        source(h.state_mut()).state.accepted,
        "initial consent click did not take effect: {:?}",
        source(h.state_mut()).state
    );
    let before = h.state().editor.document_snapshot();
    let key = source(h.state_mut()).font_files;
    pick(h.state_mut(), &ctx, None);
    h.run();
    assert!(source(h.state_mut()).state.accepted);
    let bad = root.join("bad.ttf");
    std::fs::write(&bad, b"not a font").unwrap();
    pick(h.state_mut(), &ctx, Some(bad));
    h.run();
    assert!(source(h.state_mut()).state.accepted);
    assert_eq!(source(h.state_mut()).font_files, key);
    pick(h.state_mut(), &ctx, Some(root.join("x".repeat(4097))));
    h.run();
    assert!(source(h.state_mut()).state.accepted);
    assert_eq!(source(h.state_mut()).font_files, key);
    h.state_mut().svg_font_picker = Some(FontPick {
        source: Uuid::new_v4(),
        files: key,
    });
    h.state_mut()
        .complete_svg_font_picker(&ctx, Some(root.join("must-not-be-read.ttf")));
    h.run();
    assert_eq!(source(h.state_mut()).font_files, key);
    assert!(Arc::ptr_eq(&before, &h.state().editor.document_snapshot()));
    h.get_by_role_and_label(egui::Role::Button, "Remove")
        .click();
    h.run();
    assert!(source(h.state_mut()).additional.is_empty());
    assert!(!source(h.state_mut()).state.accepted);
    h.get_by_role_and_label(egui::Role::Button, "Cancel")
        .click();
    h.run();
    assert!(Arc::ptr_eq(&before, &h.state().editor.document_snapshot()));
    drop(h);
    std::fs::remove_dir_all(root).unwrap();
}

#[test]
fn svg_and_font_input_guards_follow_saved_documents_other_tabs_and_parked_workspaces() {
    let (root, image, font) = files();
    let mut h = harness(&image, Language::English);
    h.run();
    let ctx = h.ctx.clone();
    pick(h.state_mut(), &ctx, Some(font.clone()));
    h.run();
    preview(&mut h, Language::English);
    approve(&mut h, Language::English);
    let project = root.join("converted.pcnd");
    h.state_mut().save_selected(&ctx, project.clone());
    h.state_mut().editor.run_pending();
    h.state_mut().finish_save();
    assert!(
        h.state().pending_import.is_none(),
        "font source must have been imported before ownership checks"
    );
    assert_eq!(h.state().bound.as_deref(), Some(project.as_path()));
    assert!(!h.state().editor.is_dirty());
    let moved_font = root.join("renamed-font.ttf");
    let moved_image = root.join("renamed-source.svg");
    std::fs::rename(&font, &moved_font).unwrap();
    std::fs::rename(&image, &moved_image).unwrap();
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
        for (index, source) in [&moved_font, &moved_image].into_iter().enumerate() {
            let original = std::fs::read(source).unwrap();
            let alias = root.join(format!("alias-{location}-{index}.png"));
            std::fs::hard_link(source, &alias).unwrap();
            app.do_export(&alias, ImageFormat::Png, &KNOWN_LOSSES.map(str::to_owned));
            assert_eq!(
                Sha256::digest(std::fs::read(&alias).unwrap()),
                Sha256::digest(&original)
            );
            let save = alias.with_extension("pcnd");
            std::fs::hard_link(source, &save).unwrap();
            app.save_selected(&ctx, save);
            assert!(app.confirm.is_none());
        }
    }
    drop(h);
    std::fs::remove_dir_all(root).unwrap();
}

#[test]
fn pending_svg_and_font_sources_are_protected_by_actual_save_and_export_routes() {
    let (root, image, font) = files();
    let mut h = harness(&image, Language::English);
    h.run();
    let ctx = h.ctx.clone();
    pick(h.state_mut(), &ctx, Some(font.clone()));
    h.run();
    let before = h.state().editor.document_snapshot();
    for (index, path) in [&image, &font].into_iter().enumerate() {
        let original = std::fs::read(path).unwrap();
        let alias = root.join(format!("pending-{index}.png"));
        std::fs::hard_link(path, &alias).unwrap();
        h.state_mut()
            .do_export(&alias, ImageFormat::Png, &KNOWN_LOSSES.map(str::to_owned));
        assert_eq!(
            Sha256::digest(std::fs::read(&alias).unwrap()),
            Sha256::digest(&original)
        );
        let project = alias.with_extension("pcnd");
        std::fs::hard_link(path, &project).unwrap();
        h.state_mut().save_selected(&ctx, project.clone());
        h.state_mut().editor.run_pending();
        h.state_mut().finish_save();
        assert_eq!(
            Sha256::digest(std::fs::read(&project).unwrap()),
            Sha256::digest(&original)
        );
        assert!(matches!(h.state().confirm, Some(Confirm::ImportLosses)));
        assert!(Arc::ptr_eq(&before, &h.state().editor.document_snapshot()));
    }
    h.get_by_role_and_label(egui::Role::Button, "Cancel")
        .click();
    h.run();
    assert!(h.state().auxiliary_source_guards().is_empty());
    drop(h);
    std::fs::remove_dir_all(root).unwrap();
}

#[test]
fn new_open_and_close_move_source_guards_with_the_document_and_release_the_closed_owner() {
    let (root, image, font) = files();
    let mut h = harness(&image, Language::English);
    h.run();
    let ctx = h.ctx.clone();
    pick(h.state_mut(), &ctx, Some(font.clone()));
    h.run();
    preview(&mut h, Language::English);
    approve(&mut h, Language::English);
    assert!(h.state().pending_import.is_none());
    let font_tab = h.state().active_tab_id();
    let project = root.join("converted.pcnd");
    h.state_mut().save_selected(&ctx, project);
    h.state_mut().editor.run_pending();
    h.state_mut().finish_save();
    assert!(!h.state().editor.is_dirty());
    h.state_mut().new_document();
    assert!(h.state().auxiliary_sources.is_empty());
    assert_eq!(h.state().auxiliary_source_guards().len(), 2);
    let other = root.join("other.pcnd");
    std::fs::write(
        &other,
        paintcraft_formats::pcnd::write(Editor::new_document(3, 3).unwrap().document()).unwrap(),
    )
    .unwrap();
    h.state_mut().open_path(&other).unwrap();
    assert!(h.state().auxiliary_sources.is_empty());
    assert_eq!(h.state().auxiliary_source_guards().len(), 2);
    h.state_mut().request_close_tab(font_tab, &ctx);
    assert!(!h.state().tabs.iter().any(|tab| tab.id == font_tab));
    assert!(h.state().auxiliary_source_guards().is_empty());
    for path in [&image, &font] {
        let alias = root.join(format!(
            "released-{}.png",
            path.file_stem().unwrap().to_string_lossy()
        ));
        std::fs::hard_link(path, &alias).unwrap();
        assert!(h.state().guard_export_destination(&alias).is_ok());
    }
    while h.state().document_count() > 0 {
        let id = h.state().active_tab_id();
        h.state_mut().request_close_tab(id, &ctx);
        assert!(h.state().tab_closing.is_none());
    }
    assert!(h.state().auxiliary_sources.is_empty());
    assert!(h.state().imported_source.is_none());
    drop(h);
    std::fs::remove_dir_all(root).unwrap();
}
