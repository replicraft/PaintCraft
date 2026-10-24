use super::*;
use paintcraft_core::types::{Error, ErrorCode};
use std::time::Instant;

fn editor() -> controller::Editor {
    controller::Editor::new_document(32, 24).unwrap()
}
fn pixel(frame: &DisplayFrame, x: usize, y: usize) -> [u8; 4] {
    let width = frame.request.size()[0] as usize;
    frame.pixels[(y * width + x) * 4..(y * width + x + 1) * 4]
        .try_into()
        .unwrap()
}

#[test]
fn live_paint_shape_and_bitmap_text_match_release_without_publishing() {
    for tool in [
        Tool::Pencil,
        Tool::Brush,
        Tool::Eraser,
        Tool::Line,
        Tool::Rect,
        Tool::Ellipse,
        Tool::Text,
    ] {
        let mut editor = editor();
        if tool == Tool::Eraser {
            editor.options.pencil_size = 12;
            editor.pointer_down((10, 10));
            editor.pointer_up();
            editor.run_pending();
        }
        editor.tool = tool;
        editor.options.size = 8;
        editor.options.eraser_color = Rgba([255; 4]);
        editor.options.style = controller::ShapeStyle::Both;
        editor.color = Rgba([32, 64, 96, 192]);
        editor.secondary = Rgba([128, 30, 60, 128]);
        editor.pointer_down((8, 8));
        editor.pointer_move((12, 8));
        if tool == Tool::Text {
            editor.type_text("A");
        }
        let doc = editor.document_snapshot();
        let history = editor.history_view();
        let frame = Source::capture(&editor).render(|| Ok(())).unwrap();
        assert_eq!(pixel(&frame, 0, 0), [255; 4]);
        if tool == Tool::Brush {
            assert_eq!(pixel(&frame, 10, 8), [87, 111, 135, 255]);
        }
        if tool == Tool::Eraser {
            assert_eq!(pixel(&frame, 10, 8), [255; 4]);
        }
        assert!(Arc::ptr_eq(&editor.document_snapshot(), &doc));
        assert_eq!(editor.history_view(), history);
        if tool == Tool::Text {
            editor.commit_text();
        } else {
            editor.pointer_up();
        }
        editor.run_pending();
        assert_eq!(
            frame.pixels,
            paintcraft_render::render_flat(editor.document()).unwrap(),
            "{tool:?}"
        );
    }
}

#[test]
fn live_font_text_is_private_frozen_and_matches_the_committed_mask() {
    let fonts = Arc::new(std::sync::Mutex::new(crate::text::TextRenderer::bundled()));
    let mut editor = editor().with_text_renderer(fonts);
    editor.tool = Tool::Text;
    editor.pointer_down((1, 1));
    editor.type_text("A");
    let captured = Source::capture(&editor);
    let a_scope = captured.scope;
    editor.type_text("B");
    let ab = Source::capture(&editor);
    assert_ne!(a_scope, ab.scope);
    assert!(a_scope.can_progress_to(ab.scope));
    let a = captured.render(|| Ok(())).unwrap();
    let ab = ab.render(|| Ok(())).unwrap();
    assert_ne!(a.pixels, ab.pixels);
    assert_eq!(editor.publications(), 0);
    editor.commit_text();
    editor.run_pending();
    assert_eq!(
        ab.pixels,
        paintcraft_render::render_flat(editor.document()).unwrap()
    );
}

#[test]
fn scope_rejects_other_epochs_documents_owners_and_cancelled_drafts() {
    let mut editor = editor();
    editor.pointer_down((2, 2));
    let a = Source::capture(&editor).scope;
    editor.pointer_move((3, 3));
    let b = Source::capture(&editor).scope;
    assert_ne!(a, b);
    assert!(a.can_progress_to(b));
    assert!(!a.can_progress_to(Scope {
        epoch: a.epoch + 1,
        ..b
    }));
    assert!(!a.can_progress_to(Scope {
        document: Uuid::new_v4(),
        ..b
    }));
    editor.cancel_gesture();
    let cancelled = Source::capture(&editor).scope;
    assert!(!a.can_progress_to(cancelled));
    editor.pointer_down((2, 2));
    let fresh = Source::capture(&editor).scope;
    assert!(
        !a.can_progress_to(fresh),
        "new gesture at identical coordinates has a different owner"
    );
}

fn drain(
    service: &mut Thumbnail,
    ctx: &egui::Context,
    editor: &controller::Editor,
    budget: &paintcraft_runtime::SharedBudget,
) {
    let deadline = Instant::now() + Duration::from_secs(5);
    while service.pending.is_some() {
        service.update_source(ctx, Some(Source::capture(editor)), budget, false);
        assert!(Instant::now() < deadline);
        std::thread::yield_now();
    }
}

#[test]
fn motion_coalesces_useful_frames_but_never_marks_an_old_frame_current() {
    let ctx = egui::Context::default();
    let budget = paintcraft_runtime::SharedBudget::new(3 << 30);
    let mut editor = editor();
    editor.pointer_down((2, 2));
    let source = Source::capture(&editor);
    let first = source.scope;
    let mut executor = Executor::with_budget(
        WorkerLimits {
            workers: 1,
            pending_jobs: 1,
            transient_bytes: SOURCE_LIMIT + PREVIEW_BYTES + WORK_BYTES,
        },
        budget.clone(),
    )
    .unwrap();
    let (release, gate) = std::sync::mpsc::channel();
    let handle = executor
        .submit(
            WORK_BYTES,
            move |_| {
                gate.recv_timeout(Duration::from_secs(5)).unwrap();
                source.render(|| Ok(()))
            },
            || {},
        )
        .unwrap();
    let mut service = Thumbnail {
        scope: Some(first),
        executor: Some(executor),
        pending: Some((first, handle)),
        output: Some(budget.reserve(32 * 24 * 12).unwrap()),
        ..Default::default()
    };
    editor.pointer_move((10, 10));
    let newest = Source::capture(&editor).scope;
    service.update_source(&ctx, Some(Source::capture(&editor)), &budget, true);
    assert_eq!(service.pending, Some((first, handle)));
    release.send(()).unwrap();
    drain(&mut service, &ctx, &editor, &budget);
    assert_eq!(service.image.as_ref().unwrap().scope, first);
    assert_eq!(service.scope, Some(newest));
    assert!(service.updating());
    service.update_source(&ctx, Some(Source::capture(&editor)), &budget, true);
    drain(&mut service, &ctx, &editor, &budget);
    assert_eq!(service.image.as_ref().unwrap().scope, newest);
    assert!(!service.updating());
    assert_eq!(budget.usage().reserved, 32 * 24 * 12);
    editor.cancel_gesture();
    service.update_source(&ctx, Some(Source::capture(&editor)), &budget, false);
    assert!(
        service.image.is_none(),
        "cancelled draft must not remain as a compatible prefix"
    );
    assert_eq!(budget.usage().reserved, 0);
}

#[test]
fn cancelled_and_denied_staging_never_publish_document_or_overview_pixels() {
    let ctx = egui::Context::default();
    let mut editor = editor();
    editor.pointer_down((2, 2));
    editor.pointer_move((10, 10));
    let doc = editor.document_snapshot();
    let source = Source::capture(&editor);
    assert_eq!(
        source
            .render(|| Err(Error::new(ErrorCode::OperationCancelled, "test")))
            .unwrap_err()
            .code,
        ErrorCode::OperationCancelled
    );
    let budget = paintcraft_runtime::SharedBudget::new(PREVIEW_BYTES - 1);
    let mut service = Thumbnail::default();
    service.update_source(&ctx, Some(Source::capture(&editor)), &budget, true);
    assert!(service.pending.is_none() && service.image.is_none());
    assert_eq!(budget.usage().reserved, 0);
    assert!(Arc::ptr_eq(&doc, &editor.document_snapshot()));
    assert_eq!(editor.publications(), 0);
}
