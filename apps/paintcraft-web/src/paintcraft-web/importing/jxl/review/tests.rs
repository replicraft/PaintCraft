use super::*;
use egui_kittest::{
    Harness,
    kittest::{NodeT, Queryable},
};
use paintcraft_core::types::resources::SharedBudget;
use paintcraft_ui::i18n::{self, Language, translate};

const PIXELS: [u8; 16] = [
    0, 0, 255, 1, 1, 127, 128, 0, 255, 4, 8, 128, 16, 32, 64, 255,
];
fn prepared(budget: SharedBudget) -> Prepared {
    let mut admission = paintcraft_raster::Admission::with_residency(
        paintcraft_core::types::policy::STAGED_PIXELS_MAX,
        budget,
    );
    let document = paintcraft_document::import_image_with_admission_checked(
        2,
        2,
        &PIXELS,
        "fixture",
        &mut admission,
        &|| Ok(()),
    )
    .unwrap();
    Prepared {
        document,
        name: "integer.jxl".into(),
        report: wire::jxl_integer::Report {
            source_depth: 16,
            width: 2,
            height: 2,
            source_sha256: [37; 32],
            output_sha256: Sha256::digest(PIXELS).into(),
            changed_rgb_components: 0,
            changed_alpha: 0,
        },
    }
}
fn app(ctx: &egui::Context) -> WebApp {
    let mut app = WebApp::new(ctx);
    app.frontend = paintcraft_ui::actions::Frontend::Studio;
    app
}
#[test]
fn only_a_visible_previous_frame_enables_consent() {
    let ctx = egui::Context::default();
    let app = app(&ctx);
    let mut review = Review::new(
        &ctx,
        Owner::capture(&app),
        prepared(SharedBudget::new(1 << 20)),
    )
    .unwrap();
    review.consent = true;
    let mut ready = false;
    let mut output = ctx.run_ui(Default::default(), |ui| {
        ui.set_clip_rect(egui::Rect::NOTHING);
        ready = review.show(ui);
    });
    output.textures_delta.clear();
    assert!(!ready);
    assert!(review.shown.is_none());
    let mut output = ctx.run_ui(Default::default(), |ui| ready = review.show(ui));
    output.textures_delta.clear();
    assert!(!ready);
    let mut output = ctx.run_ui(Default::default(), |ui| ready = review.show(ui));
    output.textures_delta.clear();
    assert!(ready);
}
#[test]
fn mandatory_precision_consent_binds_the_complete_report_even_with_zero_changed_counts() {
    let ctx = egui::Context::default();
    let mut app = app(&ctx);
    for change in [0, 1, 2, 3] {
        let owner = Owner::capture(&app);
        let before = app.editor.session.snapshot();
        let ticket = app.editor.session.ticket();
        let budget = SharedBudget::new(1 << 20);
        let mut value = prepared(budget.clone());
        let digest = value.assessment_digest().unwrap();
        let consent = if change == 0 { None } else { Some(digest) };
        if change == 1 {
            value.report.changed_alpha = 1;
        }
        if change == 2 {
            value.report.source_sha256[0] ^= 1;
        }
        if change == 3 {
            value.report.source_depth = 8;
        }
        app.install_jxl(owner, value, consent, &ctx);
        assert!(Arc::ptr_eq(&before, &app.editor.session.snapshot()));
        assert_eq!(ticket, app.editor.session.ticket());
        assert_eq!(budget.usage().reserved, 0);
    }
}
#[test]
fn cancellation_and_stale_review_retire_private_tiles_without_changing_the_open_project() {
    let ctx = egui::Context::default();
    let mut app = app(&ctx);
    let before = app.editor.session.snapshot();
    let budget = SharedBudget::new(1 << 20);
    app.jxl.review =
        Some(Review::new(&ctx, Owner::capture(&app), prepared(budget.clone())).unwrap());
    assert!(app.jxl.busy());
    assert!(budget.usage().reserved > 0);
    app.jxl.cancel();
    assert!(!app.jxl.busy());
    assert_eq!(budget.usage().reserved, 0);
    assert!(Arc::ptr_eq(&before, &app.editor.session.snapshot()));
    app.jxl.review =
        Some(Review::new(&ctx, Owner::capture(&app), prepared(budget.clone())).unwrap());
    app.editor = crate::editor::Editor::new(9, 7).unwrap();
    let newer = app.editor.session.snapshot();
    app.poll_jxl(&ctx);
    assert!(!app.jxl.busy());
    assert_eq!(budget.usage().reserved, 0);
    assert!(Arc::ptr_eq(&newer, &app.editor.session.snapshot()));
}
#[test]
fn interface_skin_composition_gesture_and_replaced_session_invalidate_review() {
    let ctx = egui::Context::default();
    for case in 0..5 {
        let mut candidate = app(&ctx);
        let budget = SharedBudget::new(1 << 20);
        candidate.jxl.review =
            Some(Review::new(&ctx, Owner::capture(&candidate), prepared(budget.clone())).unwrap());
        match case {
            0 => candidate.frontend = paintcraft_ui::actions::Frontend::Classic,
            1 => candidate.variant = ClassicVariant::V7,
            2 => candidate.test_start_preedit("composition"),
            3 => {
                candidate.editor.tools.tool = paintcraft_tools::Tool::Pencil;
                candidate
                    .editor
                    .tools
                    .pointer_down(candidate.editor.target.unwrap(), (1, 1));
            }
            _ => {
                let document = candidate.editor.session.document().clone();
                candidate.editor = crate::editor::Editor::from_document(document).unwrap();
            }
        }
        let before = candidate.editor.session.snapshot();
        let ticket = candidate.editor.session.ticket();
        candidate.poll_jxl(&ctx);
        assert!(!candidate.jxl.busy(), "case{case}");
        assert_eq!(budget.usage().reserved, 0);
        assert!(Arc::ptr_eq(&before, &candidate.editor.session.snapshot()));
        assert_eq!(ticket, candidate.editor.session.ticket());
    }
}
#[test]
fn translated_visible_preview_consent_adopts_exact_hidden_rgb_and_survives_pcnd_reopen() {
    for language in [Language::English, Language::Japanese] {
        let ctx = egui::Context::default();
        let app = app(&ctx);
        let observed = Rc::new(RefCell::new(None));
        let context = observed.clone();
        let mut h = Harness::builder()
            .with_size(egui::vec2(1360.0, 900.0))
            .build_ui_state(
                move |ui, app: &mut WebApp| {
                    *context.borrow_mut() = Some(ui.ctx().clone());
                    if ui.ctx().missing_glyph_policy()
                        != egui::epaint::text::MissingGlyphPolicy::Tofu
                    {
                        paintcraft_ui::fonts::install(ui.ctx());
                        ui.ctx()
                            .set_missing_glyph_policy(egui::epaint::text::MissingGlyphPolicy::Tofu);
                    }
                    i18n::set_language(ui.ctx(), language);
                    app.jxl_dialog(ui.ctx());
                },
                app,
            );
        let before = h.state().editor.session.snapshot();
        let value = prepared(SharedBudget::new(1 << 20));
        let ctx = observed.borrow().as_ref().unwrap().clone();
        let review = Review::new(&ctx, Owner::capture(h.state()), value).unwrap();
        h.state_mut().jxl.review = Some(review);
        for _ in 0..6 {
            h.step();
            if h.state().jxl.review.as_ref().unwrap().shown.is_some() {
                break;
            }
        }
        assert!(h.state().jxl.review.as_ref().unwrap().shown.is_some());
        let loss =
            paintcraft_ui::i18n::import_loss_description(paintcraft_formats::jxl::DEPTH16_LOSS)
                .unwrap();
        h.get_by_label(&translate(language, loss));
        assert!(
            h.get_by_role_and_label(egui::Role::Button, &translate(language, "Open"))
                .accesskit_node()
                .is_disabled()
        );
        assert!(Arc::ptr_eq(&before, &h.state().editor.session.snapshot()));
        h.step();
        h.get_by_label(&translate(language, "Accept import limitations"))
            .click();
        h.run();
        h.get_by_role_and_label(egui::Role::Button, &translate(language, "Open"))
            .click();
        h.run();
        assert!(!h.state().jxl.busy());
        assert!(h.state().editor.session.is_dirty());
        assert_eq!(h.state().editor.session.undo_depth(), 0);
        assert_eq!(h.state().download_name(None), "integer.pcnd");
        let bytes =
            paintcraft_formats::pcnd::write_compact(h.state().editor.session.document()).unwrap();
        let reopened = paintcraft_formats::pcnd::read(&bytes).unwrap();
        let raster = &reopened.layers_in_order().next().unwrap().raster;
        for (index, pixel) in PIXELS.as_chunks::<4>().0.iter().enumerate() {
            assert_eq!(&raster.get((index % 2) as i64, (index / 2) as i64).0, pixel);
        }
    }
}
