use super::*;
use paintcraft_commands::{LayerMaskOp, LayerRef};
use paintcraft_core::types::{Rect, json::Value};
use std::sync::Arc;
fn setup() -> (egui::Context, WebApp) {
    let ctx = egui::Context::default();
    let mut app = WebApp::new(&ctx);
    app.frontend = Frontend::Studio;
    app.editor = Editor::new(4, 1).unwrap();
    app.editor
        .commit(vec![Value::object([
            ("op", Value::str("selection.set_rect")),
            (
                "rect",
                Value::object([
                    ("x", Value::Int(1)),
                    ("y", Value::Int(0)),
                    ("width", Value::Int(2)),
                    ("height", Value::Int(1)),
                ]),
            ),
        ])])
        .unwrap();
    app.canvas.filter_view = Some(
        View::new(
            app.editor.session.document(),
            Rect::new(0, 0, 4, 1).unwrap(),
            0,
        )
        .unwrap(),
    );
    (ctx, app)
}
fn capture(app: &WebApp) -> LayerMaskEdit {
    app.editor.layer_mask_capture(LayerMaskOp::FromSelection {
        layer: LayerRef::Id(app.editor.target.unwrap()),
        enabled: true,
    })
}
fn request(source: &Capture, view: View) -> Request {
    Request {
        revision: Revision {
            draft: source.draft,
            generation: 1,
        },
        edit: None,
        render: true,
        view,
    }
}
#[test]
fn published_own_mask_retires_old_display_identity_and_reopens_source7_without_numeric_intent() {
    let (ctx, mut app) = setup();
    let source = app.filter_capture();
    let key = channel::SourceKey::new(source.clone(), None, None).unwrap();
    assert!(matches!(&key, channel::SourceKey::Legacy(_)));
    app.filter_display.source = Some(source.clone());
    app.filter_display.committed = Some(source.clone());
    app.filter_display.renderer.accepted = Some(request(&source, app.canvas.filter_view.unwrap()));
    let before = app.editor.session.ticket();
    app.apply_own_layer_mask(&ctx, capture(&app));
    assert_ne!(app.editor.session.ticket(), before);
    assert!(app.filter_display.source.is_none());
    assert!(app.filter_display.committed.is_none());
    assert!(app.filter_display.renderer.accepted.is_none());
    assert!(app.canvas.filter_footprint.is_none());
    let source = app.filter_capture();
    let updated = channel::SourceKey::new(source.clone(), None, None).unwrap();
    assert!(matches!(&updated, channel::SourceKey::Appearance(_)));
    assert!(!key.same(&updated));
    // A repeated semantic no-op must not close/reopen the still-valid source.
    let ready = request(&source, app.canvas.filter_view.unwrap());
    app.filter_display.source = Some(source);
    app.filter_display.renderer.accepted = Some(ready.clone());
    let before = app.editor.session.snapshot();
    app.apply_own_layer_mask(&ctx, capture(&app));
    assert!(Arc::ptr_eq(&before, &app.editor.session.snapshot()));
    assert_eq!(app.filter_display.renderer.accepted, Some(ready));
    assert!(app.filter_display.source.is_some());
}
#[test]
fn active_private_text_and_late_capture_do_not_discard_drafts_or_display() {
    let (ctx, mut app) = setup();
    let late = capture(&app);
    app.editor
        .commit(vec![Value::object([("op", Value::str("selection.clear"))])])
        .unwrap();
    let source = app.filter_capture();
    app.filter_display.source = Some(source);
    let before = app.editor.session.snapshot();
    let ticket = app.editor.session.ticket();
    app.apply_own_layer_mask(&ctx, late);
    assert!(Arc::ptr_eq(&before, &app.editor.session.snapshot()));
    assert_eq!(app.editor.session.ticket(), ticket);
    assert!(app.filter_display.source.is_some());
    app.editor
        .tools
        .begin_text(app.editor.target.unwrap(), (0, 0));
    app.editor.tools.type_text("still editable");
    let stamp = app.editor.tools.text_stamp();
    app.apply_own_layer_mask(&ctx, capture(&app));
    assert_eq!(app.editor.tools.text_stamp(), stamp);
    assert!(Arc::ptr_eq(&before, &app.editor.session.snapshot()));
    assert!(app.filter_display.source.is_some());
}
#[test]
fn own_mask_statuses_and_errors_are_localized_in_every_configured_language() {
    use paintcraft_ui::i18n::{Language, set_language};
    for language in Language::ALL {
        let (ctx, mut app) = setup();
        set_language(&ctx, language);
        app.apply_own_layer_mask(&ctx, capture(&app));
        assert_eq!(app.message, t!(&ctx, "Layer mask updated"));
        if language != Language::English {
            assert_ne!(app.message, "Layer mask updated");
        }
        app.apply_own_layer_mask(&ctx, capture(&app));
        assert_eq!(app.message, t!(&ctx, "Layer mask unchanged"));
        let mut stale = capture(&app);
        stale.owner = Uuid::new_v4();
        app.apply_own_layer_mask(&ctx, stale);
        let detail = t!(&ctx, "Layer mask source or target changed");
        assert_eq!(
            app.message,
            t!(&ctx, "Edit failed: { $detail }", detail = detail)
        );
        if language != Language::English {
            assert_ne!(detail, "Layer mask source or target changed");
        }
    }
}

#[test]
fn all_modes_publish_mask_clear_legacy_cache_and_preserve_legacy_normal_guard() {
    use paintcraft_core::types::LayerBlendMode;
    for mode in [
        LayerBlendMode::Normal,
        LayerBlendMode::Multiply,
        LayerBlendMode::Screen,
    ] {
        let (ctx, mut app) = setup();
        let mut document = (*app.editor.session.snapshot()).clone();
        document
            .layers
            .get_mut(&document.roots[0])
            .unwrap()
            .blend_mode = mode;
        app.editor = Editor::from_source8_document(document).unwrap();
        app.editor
            .commit(vec![Value::object([
                ("op", Value::str("selection.set_rect")),
                (
                    "rect",
                    Value::object([
                        ("x", Value::Int(1)),
                        ("y", Value::Int(0)),
                        ("width", Value::Int(2)),
                        ("height", Value::Int(1)),
                    ]),
                ),
            ])])
            .unwrap();
        // Only legacy cache invalidation is native here; the actual Source8
        // callback/retirement lifecycle remains a required browser gate.
        let source = app.filter_capture();
        app.filter_display.source = Some(source.clone());
        app.filter_display.committed = Some(source);
        let before = app.editor.session.snapshot();
        let ticket = app.editor.session.ticket();
        app.apply_own_layer_mask(&ctx, capture(&app));
        let masked = app.editor.session.snapshot();
        assert!(!Arc::ptr_eq(&before, &masked));
        assert_ne!(app.editor.session.ticket(), ticket);
        assert_eq!(masked.layers[&masked.roots[0]].blend_mode, mode);
        assert!(masked.layers[&masked.roots[0]].mask.is_some());
        assert!(app.filter_display.source.is_none());
        assert!(app.filter_display.committed.is_none());
        assert!(app.canvas.filter_footprint.is_none());
        if mode == LayerBlendMode::Normal {
            let key = channel::SourceKey::new(app.filter_capture(), None, None).unwrap();
            assert!(matches!(key, channel::SourceKey::Appearance(_)));
            assert_eq!(snapshot::encode(&masked, &|| Ok(())).unwrap()[4], 7);
        } else {
            assert_eq!(
                snapshot::encode(&masked, &|| Ok(())).unwrap_err().code,
                ErrorCode::CapabilityUnsupported
            );
            assert_eq!(
                paintcraft_formats::pcnd::write_compact(&masked).unwrap()[4],
                8
            );
        }
        let ticket = app.editor.session.ticket();
        app.apply_own_layer_mask(&ctx, capture(&app));
        assert!(Arc::ptr_eq(&masked, &app.editor.session.snapshot()));
        assert_eq!(app.editor.session.ticket(), ticket);
        assert_eq!(app.message, t!(&ctx, "Layer mask unchanged"));
    }
}
