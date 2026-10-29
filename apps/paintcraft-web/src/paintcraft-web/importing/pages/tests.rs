use super::*;
use crate::editor::Editor;

fn fixture() -> Vec<u8> {
    let mut bytes = vec![0; 4100];
    bytes[..4].copy_from_slice(&987_654_321u32.to_le_bytes());
    for (index, rgb) in [[10, 20, 30], [200, 210, 220]].into_iter().enumerate() {
        let offset = bytes.len() as u32;
        bytes[4 + index * 4..8 + index * 4].copy_from_slice(&offset.to_le_bytes());
        let mut pcx = vec![0; 128];
        pcx[..4].copy_from_slice(&[10, 5, 1, 8]);
        pcx[65] = 3;
        pcx[66..68].copy_from_slice(&2u16.to_le_bytes());
        for value in rgb {
            if value >= 192 {
                pcx.extend([0xc1, value]);
            } else {
                pcx.push(value);
            }
            pcx.push(0);
        }
        bytes.extend(pcx);
    }
    bytes
}
fn stage(app: &mut WebApp, ctx: &egui::Context) {
    app.frontend = paintcraft_ui::actions::Frontend::Studio;
    app.import_owner = Some(Owner::capture(app));
    app.accept_pages(
        Source::new(fixture(), "pages.dcx".into(), Frontend::Studio).unwrap(),
        ctx,
    );
}
#[test]
fn explicit_page_and_separate_loss_consent_preserve_original_until_accept() {
    let ctx = egui::Context::default();
    let mut app = WebApp::new(&ctx);
    stage(&mut app, &ctx);
    let source = app.editor.session.snapshot();
    let ticket = app.editor.session.ticket();
    assert_eq!(app.pending_pages.as_ref().unwrap().selected, None);
    assert_eq!(app.pending_pages.as_ref().unwrap().source.pages.len(), 2);
    app.choose_page_import(&ctx, 9);
    assert!(app.pending_pages.as_ref().unwrap().error.is_some());
    assert!(app.pending_import.is_none());
    assert!(Arc::ptr_eq(&source, &app.editor.session.snapshot()));
    app.choose_page_import(&ctx, 1);
    assert!(app.pending_pages.is_none());
    assert!(app.pending_import.is_some());
    assert!(Arc::ptr_eq(&source, &app.editor.session.snapshot()));
    assert_eq!(ticket, app.editor.session.ticket());
    app.resolve_import(&ctx, false);
    assert!(Arc::ptr_eq(&source, &app.editor.session.snapshot()));
    stage(&mut app, &ctx);
    app.choose_page_import(&ctx, 1);
    app.resolve_import(&ctx, true);
    let doc = app.editor.session.document();
    assert_eq!(doc.canvas, (1, 1));
    assert_eq!(
        doc.layers[&doc.roots[0]].raster.get(0, 0).0,
        [200, 210, 220, 255]
    );
    assert!(
        registry::admit_open(Frontend::Studio, "pages.dcx", &fixture(), Platform::Web).is_err()
    );
}
#[test]
fn cancelled_stale_or_foreign_owner_cannot_publish_a_page() {
    let ctx = egui::Context::default();
    for case in 0..5 {
        let mut app = WebApp::new(&ctx);
        stage(&mut app, &ctx);
        match case {
            0 => app.cancel_page_import(&ctx),
            1 => app.frontend = paintcraft_ui::actions::Frontend::Classic,
            2 => app.editor = Editor::new(3, 3).unwrap(),
            3 => app.text.preedit = "composition".into(),
            _ => app.variant = paintcraft_classic::ClassicVariant::V7,
        }
        let before = app.editor.session.snapshot();
        let ticket = app.editor.session.ticket();
        app.choose_page_import(&ctx, 1);
        app.resolve_import(&ctx, true);
        assert!(
            Arc::ptr_eq(&before, &app.editor.session.snapshot()),
            "case {case}"
        );
        assert_eq!(ticket, app.editor.session.ticket());
        assert!(app.pending_pages.is_none());
        assert!(app.pending_import.is_none());
    }
}
#[test]
fn actual_page_dialog_requires_explicit_selection_and_then_separate_consent() {
    use egui_kittest::{
        Harness,
        kittest::{NodeT, Queryable},
    };
    let mut h = Harness::builder()
        .with_size(egui::vec2(1360.0, 900.0))
        .build_eframe(|cc| {
            let mut app = WebApp::new(&cc.egui_ctx);
            stage(&mut app, &cc.egui_ctx);
            app
        });
    h.run();
    let before = h.state().editor.session.snapshot();
    assert!(
        h.get_by_label("Import selected page")
            .accesskit_node()
            .is_disabled()
    );
    h.get_by_label("Page 2: 1 × 1 px").click();
    h.run();
    h.get_by_role_and_label(egui::Role::Button, "Import selected page")
        .click();
    h.run();
    assert!(h.state().pending_pages.is_none());
    assert!(h.state().pending_import.is_some());
    assert!(Arc::ptr_eq(&before, &h.state().editor.session.snapshot()));
    h.get_by_role_and_label(egui::Role::Button, "Open image")
        .click();
    h.run();
    assert_eq!(h.state().editor.session.document().canvas, (1, 1));
}

#[test]
fn oversized_page_stays_disabled_and_cancel_drops_private_source() {
    use egui_kittest::{
        Harness,
        kittest::{NodeT, Queryable},
    };
    let mut app = WebApp::new(&egui::Context::default());
    stage(&mut app, &egui::Context::default());
    let pending = app.pending_pages.as_mut().unwrap();
    pending.source.pages[1].width = 4_194_305;
    let mut h = Harness::builder()
        .with_size(egui::vec2(1360.0, 900.0))
        .build_ui_state(|ui, app: &mut WebApp| app.show(ui), app);
    h.run();
    let before = h.state().editor.session.snapshot();
    h.get_by_label("Page 2: 4194305 × 1 px").click();
    h.run();
    assert!(
        h.get_by_label("Import selected page")
            .accesskit_node()
            .is_disabled()
    );
    h.get_by_role_and_label(egui::Role::Button, "Cancel")
        .click();
    h.run();
    assert!(h.state().pending_pages.is_none());
    assert!(h.state().pending_import.is_none());
    assert!(Arc::ptr_eq(&before, &h.state().editor.session.snapshot()));
}

#[test]
fn selected_pages_have_distinct_stable_document_state_and_layer_identities() {
    let ctx = egui::Context::default();
    let import = |page| {
        let mut app = WebApp::new(&ctx);
        stage(&mut app, &ctx);
        app.choose_page_import(&ctx, page);
        app.resolve_import(&ctx, true);
        app.editor.session.document().clone()
    };
    let first = import(0);
    let second = import(1);
    let repeat = import(1);
    assert_ne!(
        first.layers[&first.roots[0]].raster.get(0, 0),
        second.layers[&second.roots[0]].raster.get(0, 0)
    );
    assert_ne!(
        first.document_id, second.document_id,
        "different selected pixels cannot share a document ID"
    );
    assert_ne!(
        first.state_id, second.state_id,
        "render caches require distinct states"
    );
    assert_ne!(first.roots, second.roots);
    assert_eq!(second.document_id, repeat.document_id);
    assert_eq!(second.state_id, repeat.state_id);
    assert_eq!(second.roots, repeat.roots);
}
