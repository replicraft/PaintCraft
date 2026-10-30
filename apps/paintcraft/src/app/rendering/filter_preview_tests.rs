//! Host canvas pixels, not just preview admission predicates.
use super::*;
use paintcraft_core::types::{Rgba, filters::Descriptor, resources::SharedBudget};
use paintcraft_document::{FilterInstance, FilterStack};
use paintcraft_input::{Modifiers, tablet::*};
const BG: [u8; 4] = [10, 30, 50, 255];
const INK: [u8; 4] = [40, 80, 120, 255];
// Invert(INK) over BG with layer opacity128, default-v1 integer compositing.
const COMPOSITED: [u8; 4] = [113, 103, 93, 255];
#[derive(Clone, Copy, Debug)]
enum Kind {
    Sampled,
    Pen,
    Resource,
    Move,
    Brush,
    Shape,
    Text,
}
const ALL: [Kind; 7] = [
    Kind::Sampled,
    Kind::Pen,
    Kind::Resource,
    Kind::Move,
    Kind::Brush,
    Kind::Shape,
    Kind::Text,
];
fn sample(sequence: u64, phase: Phase) -> Sample {
    Sample {
        identity: Identity {
            device: 1,
            pointer: 2,
            kind: DeviceKind::Pen,
        },
        sequence,
        time: Time {
            epoch: 1,
            micros: sequence * 100,
            clock: Clock::HostReceipt,
        },
        phase,
        position: if sequence == 1 { (8., 8.) } else { (15., 8.) },
        transform: Transform {
            owner: 3,
            epoch: 4,
            origin: (0., 0.),
            points_per_pixel: 1.,
        },
        buttons: Buttons(Buttons::TIP),
        modifiers: Modifiers::default(),
        pressure: Some(1.),
        tilt: None,
        twist_degrees: None,
        contact: None,
        provenance: Provenance::Actual,
    }
}
fn setup(kind: Kind, asynchronous: bool) -> App {
    let mut doc =
        paintcraft_document::new_document(48, 32, Some(Rgba(BG)), 0, paintcraft_document::POLICY)
            .unwrap();
    let mut rgba = vec![0; 48 * 32 * 4];
    if matches!(kind, Kind::Move) {
        rgba[(8 * 48 + 8) * 4..(8 * 48 + 8) * 4 + 4].copy_from_slice(&INK);
    }
    let top =
        paintcraft_document::import_image(48, 32, &rgba, "native-filter-preview-top/v1").unwrap();
    let mut layer = top.layers[&top.roots[0]].clone();
    layer.opacity = 128;
    layer.filters = FilterStack::new(vec![FilterInstance::new(
        Uuid::from_u128(1),
        true,
        Descriptor::InvertRgbV1,
    )])
    .unwrap();
    doc.roots.push(layer.id);
    assert!(doc.layers.insert(layer.id, layer).is_none());
    let mut app = App::new(None);
    app.settings.experience = Experience::Studio;
    app.editor = Editor::with_document(doc, None).with_text_renderer(app.text_renderer.clone());
    app.editor.color = Rgba(INK);
    app.editor.options.size = 1;
    app.editor.options.eraser_color = Rgba::TRANSPARENT;
    if matches!(kind, Kind::Sampled) {
        let tip = paintcraft_raster::sampled_brush::SampledTip::from_alpha(
            1,
            1,
            (0, 0),
            &[255],
            &SharedBudget::new(1 << 20),
            &|| Ok(()),
        )
        .unwrap();
        app.editor.use_sampled_tip(Some((tip, 256)));
    }
    if matches!(kind, Kind::Resource) {
        use paintcraft_formats::csh;
        app.editor.use_resource_shape(paintcraft_tools::resource_shape::Reference{library:Uuid::from_u128(7),preset:1,policy:csh::placement::Policy::CubicFillV1.id()},Arc::new(csh::decode(include_bytes!("../../../../../crates/services/paintcraft-formats/tests/csh-fixtures/placement.csh"),&||Ok(())).unwrap()),"Lens".into()).unwrap();
    }
    if matches!(kind, Kind::Move) {
        app.editor.request(
            100,
            paintcraft_tools::Action::Select {
                corners: Some(((8, 8), (9, 9))),
            },
        );
        app.editor.run_pending();
        app.editor.select_tool(Tool::Select);
    }
    if asynchronous {
        app.initialize_editing(app.context.clone());
        app.renderer = Some(
            Executor::new(WorkerLimits {
                workers: 1,
                pending_jobs: 2,
                transient_bytes: 1 << 30,
            })
            .unwrap(),
        );
    }
    app
}
fn begin(app: &mut App, kind: Kind) {
    match kind {
        Kind::Pen => {
            app.editor.select_tool(Tool::Brush);
            app.editor.options.size = 3;
            let budget = SharedBudget::new(64 << 20);
            assert!(
                app.editor
                    .pen_sample(sample(1, Phase::Down), &budget)
                    .unwrap()
            );
            assert!(
                app.editor
                    .pen_sample(sample(2, Phase::Move), &budget)
                    .unwrap()
            );
        }
        Kind::Sampled => {
            app.editor.pointer_down((8, 8));
            app.editor.pointer_move((15, 8));
        }
        Kind::Resource => {
            app.editor.pointer_down((4, 5));
            app.editor.pointer_move((20, 17));
        }
        Kind::Move => {
            app.editor.pointer_down((8, 8));
            app.editor.pointer_move((18, 8));
        }
        Kind::Brush => {
            app.editor.select_tool(Tool::Brush);
            app.editor.options.size = 3; // A fully covered center, not a 1px AA edge.
            app.editor.pointer_down((8, 8));
            app.editor.pointer_move((15, 8));
        }
        Kind::Shape => {
            app.editor.select_tool(Tool::Rect);
            app.editor.pointer_down((8, 8));
            app.editor.pointer_move((15, 12));
        }
        Kind::Text => {
            app.editor.select_tool(Tool::Text);
            app.editor.pointer_down((4, 4));
            app.editor.pointer_up();
            app.editor.type_text("A");
        }
    }
    let preview = app.editor.preview().expect("real gesture preview");
    assert!(
        matches!(
            (kind, preview),
            (Kind::Sampled, Preview::SampledStroke(_))
                | (Kind::Pen, Preview::PenStroke(_))
                | (Kind::Resource, Preview::ResourceShape(_))
                | (Kind::Move, Preview::MoveSelection { .. })
                | (Kind::Brush, Preview::Path(_))
                | (Kind::Shape, Preview::Shape { .. })
                | (Kind::Text, Preview::Text { .. })
        ),
        "{kind:?}"
    );
}
fn cancel(app: &mut App, kind: Kind) {
    if matches!(kind, Kind::Text) {
        app.editor.cancel_text();
    } else {
        app.editor.cancel_gesture();
    }
}
fn release(app: &mut App, kind: Kind) {
    match kind {
        Kind::Pen => {
            assert!(
                app.editor
                    .pen_sample(sample(3, Phase::Up), &SharedBudget::new(64 << 20))
                    .unwrap()
            );
        }
        Kind::Text => app.editor.place_text(),
        _ => app.editor.pointer_up(),
    }
    let until = std::time::Instant::now() + Duration::from_secs(10);
    loop {
        app.editor.run_pending();
        if !app.editor.has_pending_edits() {
            break;
        }
        assert!(
            std::time::Instant::now() < until,
            "{kind:?}: {}",
            app.editor.status()
        );
        std::thread::yield_now();
    }
}
fn frame(app: &mut App, ctx: &egui::Context) {
    app.sync_texture(ctx);
    let key = app.presentation_key();
    let until = std::time::Instant::now() + Duration::from_secs(10);
    while app.texture_key != Some(key) {
        assert!(
            std::time::Instant::now() < until,
            "render stalled: {}",
            app.editor.status()
        );
        app.sync_texture(ctx);
        std::thread::yield_now();
    }
}
fn pixel(rgba: &[u8], x: usize, y: usize) -> &[u8] {
    &rgba[(y * 48 + x) * 4..(y * 48 + x) * 4 + 4]
}
#[test]
fn every_real_host_preview_matches_filtered_release_and_cancel_undo_restore_backdrop() {
    for asynchronous in [false, true] {
        for kind in ALL {
            let mut app = setup(kind, asynchronous);
            let ctx = egui::Context::default();
            frame(&mut app, &ctx);
            let source = app.editor.document_snapshot();
            let history = app.editor.history_view();
            let depth = app.editor.undo_depth();
            let dirty = app.editor.published_is_dirty();
            let baseline = app.rendered.clone();
            begin(&mut app, kind);
            frame(&mut app, &ctx);
            let preview = app.rendered.clone();
            let old_key = app.presentation_key();
            assert_ne!(
                preview, baseline,
                "{kind:?} async={asynchronous}: host omitted the raster preview"
            );
            assert_eq!(pixel(&preview, 0, 0), BG);
            if matches!(kind, Kind::Sampled | Kind::Pen | Kind::Brush) {
                assert_eq!(pixel(&preview, 8, 8), COMPOSITED, "{kind:?}");
            }
            if matches!(kind, Kind::Move) {
                assert_eq!(pixel(&preview, 18, 8), COMPOSITED);
                assert_eq!(pixel(&preview, 8, 8), BG);
            }
            assert!(Arc::ptr_eq(&source, &app.editor.document_snapshot()));
            assert_eq!(history, app.editor.history_view());
            assert_eq!(dirty, app.editor.published_is_dirty());
            if matches!(kind, Kind::Text) {
                assert!(
                    app.has_document_drafts(),
                    "authored private text remains an App draft without Session publication"
                );
            }
            cancel(&mut app, kind);
            frame(&mut app, &ctx);
            assert_eq!(app.rendered, baseline);
            assert!(!app.accept_render_key(old_key));
            assert!(Arc::ptr_eq(&source, &app.editor.document_snapshot()));
            assert_eq!(history, app.editor.history_view());
            assert_eq!(dirty, app.editor.published_is_dirty());
            begin(&mut app, kind);
            frame(&mut app, &ctx);
            assert_eq!(app.rendered, preview);
            release(&mut app, kind);
            let committed = render_flat(app.editor.document()).unwrap();
            assert_eq!(preview, committed, "{kind:?} async={asynchronous}");
            frame(&mut app, &ctx);
            assert_eq!(app.rendered, committed);
            assert_eq!(
                app.editor.undo_depth(),
                depth + 1,
                "{kind:?}: exactly one gesture entry"
            );
            assert!(app.editor.published_is_dirty());
            app.editor.request(900, paintcraft_tools::Action::Undo);
            let until = std::time::Instant::now() + Duration::from_secs(10);
            loop {
                app.editor.run_pending();
                if !app.editor.has_pending_edits() {
                    break;
                }
                assert!(std::time::Instant::now() < until);
                std::thread::yield_now();
            }
            frame(&mut app, &ctx);
            assert_eq!(app.rendered, baseline);
            assert!(app.editor.document().same_semantics(&source));
            assert_eq!(app.editor.undo_depth(), depth);
            assert_eq!(dirty, app.editor.published_is_dirty());
        }
    }
}
#[test]
fn cancelled_four_new_preview_routes_reject_queued_real_renderer_completions() {
    for kind in [Kind::Sampled, Kind::Pen, Kind::Resource, Kind::Move] {
        let mut app = setup(kind, false);
        let ctx = egui::Context::default();
        frame(&mut app, &ctx);
        let baseline = app.rendered.clone();
        let source = app.editor.document_snapshot();
        let history = app.editor.history_view();
        begin(&mut app, kind);
        frame(&mut app, &ctx);
        assert_ne!(app.rendered, baseline, "{kind:?}");
        let (send, wait) = std::sync::mpsc::channel();
        let (started, ready) = std::sync::mpsc::sync_channel(1);
        let base = app.presentation_key();
        let pixels = baseline.clone();
        let mut worker = Executor::new(WorkerLimits {
            workers: 1,
            pending_jobs: 2,
            transient_bytes: 1 << 30,
        })
        .unwrap();
        worker
            .submit(
                1024,
                move |_| {
                    started.send(()).unwrap();
                    wait.recv_timeout(Duration::from_secs(10)).unwrap();
                    Ok(RenderedFrame {
                        key: base,
                        pixels,
                        fallback: None,
                        device_epoch: None,
                        gpu_texture: None,
                    })
                },
                || {},
            )
            .unwrap();
        ready.recv_timeout(Duration::from_secs(5)).unwrap();
        app.renderer = Some(worker);
        app.texture_key = None;
        app.sync_texture(&ctx);
        let old = app.presentation_key();
        assert!(app.pending_frame.is_some());
        cancel(&mut app, kind);
        app.sync_texture(&ctx);
        assert!(!app.accept_render_key(old));
        send.send(()).unwrap();
        frame(&mut app, &ctx);
        assert_eq!(app.rendered, baseline);
        assert!(Arc::ptr_eq(&source, &app.editor.document_snapshot()));
        assert_eq!(history, app.editor.history_view());
        assert!(!app.editor.published_is_dirty());
    }
}
