use super::*;
use paintcraft_core::types::{Error, ErrorCode, clipboard::Image, resources::SharedBudget};
use paintcraft_tools::Action as Edit;

fn fixture(ctx: &egui::Context) -> WebApp {
    let mut app = WebApp::new(ctx);
    app.editor = crate::editor::Editor::new(8, 6).unwrap();
    app
}
fn decoded(losses: Vec<String>) -> clipboard::Decoded {
    clipboard::Decoded {
        image: Image::from_rgba(
            2,
            1,
            vec![11, 22, 33, 0, 44, 55, 66, 127],
            &SharedBudget::new(64),
            &|| Ok(()),
        )
        .unwrap(),
        losses,
    }
}
fn stage(app: &mut WebApp, ctx: &egui::Context, losses: Vec<String>) {
    app.paste_with(ctx, || Ok(clipboard::Job::completed(Ok(decoded(losses)))));
    app.poll_paste(ctx);
    assert!(app.paste.ready.is_some());
}
#[test]
fn paste_is_private_until_accept_and_one_undo_restores_original_document() {
    let ctx = egui::Context::default();
    let mut app = fixture(&ctx);
    let before = app.editor.session.snapshot();
    stage(&mut app, &ctx, vec![]);
    assert!(Arc::ptr_eq(&before, &app.editor.session.snapshot()));
    assert!(app.blocked());
    let mut ready = app.paste.ready.take().unwrap();
    ready.origin = (9, 7);
    ready.grow = true;
    app.accept_paste(&ctx, ready);
    assert_eq!(app.editor.session.document().canvas, (11, 8));
    let layer = app
        .editor
        .session
        .document()
        .layers
        .get(&app.editor.target.unwrap())
        .unwrap();
    assert_eq!(layer.origin, (9, 7));
    assert_eq!(layer.raster.get(0, 0).0, [11, 22, 33, 0]);
    assert_eq!(layer.raster.get(1, 0).0, [44, 55, 66, 127]);
    assert_eq!(app.editor.session.undo_depth(), 1);
    app.editor.session.undo().unwrap();
    assert!(Arc::ptr_eq(&app.editor.session.snapshot(), &before));
    app.editor.session.redo().unwrap();
    assert_eq!(app.editor.session.document().canvas, (11, 8));
    assert!(!app.paste.busy());
}
#[test]
fn failure_cancel_selection_change_and_foreign_session_never_publish() {
    for mode in 0..5 {
        let ctx = egui::Context::default();
        let mut app = fixture(&ctx);
        let original = app.editor.session.snapshot();
        app.paste_with(&ctx, || {
            Ok(clipboard::Job::completed(if mode == 0 {
                Err(Error::new(ErrorCode::StorageFailed, "denied"))
            } else {
                Ok(decoded(vec![]))
            }))
        });
        match mode {
            1 => app.cancel_paste(&ctx),
            2 => {
                app.editor
                    .apply(Edit::Select {
                        corners: Some(((1, 1), (2, 2))),
                    })
                    .unwrap();
            }
            3 => {
                app.editor = crate::editor::Editor::new(8, 6).unwrap();
            }
            4 => {
                app.variant = ClassicVariant::V7;
            }
            _ => {}
        }
        let current = app.editor.session.snapshot();
        app.poll_paste(&ctx);
        assert!(Arc::ptr_eq(&current, &app.editor.session.snapshot()));
        assert!(app.paste.ready.is_none());
        assert!(!app.paste.busy());
        if mode < 2 {
            assert!(Arc::ptr_eq(&original, &current));
        }
    }
}
#[test]
fn consent_placement_and_browser_growth_limits_are_rechecked_at_commit() {
    let ctx = egui::Context::default();
    let mut app = fixture(&ctx);
    let before = app.editor.session.snapshot();
    stage(&mut app, &ctx, vec!["depth-16-reduced-to-8".into()]);
    let ready = app.paste.ready.take().unwrap();
    app.accept_paste(&ctx, ready);
    assert!(app.paste.ready.is_some());
    let mut ready = app.paste.ready.take().unwrap();
    ready.accept_losses = true;
    ready.origin = (4095, 4095);
    ready.grow = true;
    app.accept_paste(&ctx, ready);
    assert!(app.paste.ready.is_some());
    assert!(Arc::ptr_eq(&before, &app.editor.session.snapshot()));
    let mut ready = app.paste.ready.take().unwrap();
    ready.origin = (-1, 0);
    app.accept_paste(&ctx, ready);
    assert!(app.paste.ready.is_some());
    let mut ready = app.paste.ready.take().unwrap();
    ready.grow = false;
    app.accept_paste(&ctx, ready);
    assert_eq!(app.editor.session.document().canvas, (8, 6));
    assert_eq!(app.editor.session.undo_depth(), 1);
}
#[test]
fn private_edits_and_copy_jobs_prevent_read_submission() {
    let ctx = egui::Context::default();
    let mut app = fixture(&ctx);
    app.text.preedit = "composition".into();
    app.paste_with(&ctx, || panic!("IME must prevent clipboard access"));
    assert!(!app.paste.busy());
    app.text.preedit.clear();
    app.paste.activation = None;
    app.copy_requested(&ctx); // native host cannot acknowledge a browser write
    assert!(!app.clipboard.busy());
    stage(&mut app, &ctx, vec![]);
    app.copy_requested(&ctx);
    assert!(!app.clipboard.busy());
    assert!(app.paste.ready.is_some());
}
#[test]
fn unknown_import_losses_and_stale_confirmation_are_rejected() {
    let ctx = egui::Context::default();
    let mut app = fixture(&ctx);
    app.paste_with(&ctx, || {
        Ok(clipboard::Job::completed(Ok(decoded(vec![
            "unknown-loss".into(),
        ]))))
    });
    app.poll_paste(&ctx);
    assert!(!app.paste.busy());
    app.paste.activation = None;
    stage(&mut app, &ctx, vec![]);
    let ready = app.paste.ready.take().unwrap();
    app.editor
        .apply(Edit::Select {
            corners: Some(((0, 0), (0, 0))),
        })
        .unwrap();
    let before = app.editor.session.snapshot();
    app.accept_paste(&ctx, ready);
    assert!(Arc::ptr_eq(&before, &app.editor.session.snapshot()));
    assert!(!app.paste.busy());
}
#[test]
fn modal_controls_accept_or_cancel_in_all_skins_and_two_languages() {
    use egui_kittest::{
        Harness,
        kittest::{NodeT, Queryable},
    };
    use paintcraft_ui::i18n::{Language, set_language};
    for variant in ClassicVariant::ALL {
        for language in [Language::English, Language::Japanese] {
            let mut harness = Harness::builder()
                .with_size(egui::vec2(1360.0, 900.0))
                .build_eframe(|cc| {
                    let mut app = fixture(&cc.egui_ctx);
                    app.variant = variant;
                    set_language(&cc.egui_ctx, language);
                    stage(&mut app, &cc.egui_ctx, vec![]);
                    app
                });
            harness.run();
            let cancel = variant == ClassicVariant::V7;
            let label = t!(&harness.ctx, if cancel { "Cancel" } else { "Paste" });
            harness
                .get_all_by_role_and_label(egui::Role::Button, &label)
                .into_iter()
                .find(|node| !node.accesskit_node().is_disabled())
                .unwrap()
                .click();
            harness.run();
            assert!(!harness.state().paste.busy());
            assert_eq!(
                harness.state().editor.session.undo_depth(),
                usize::from(!cancel)
            );
        }
    }
}
