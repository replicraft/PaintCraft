//! Host publication tests with explicit injected outcomes, not Worker claims.
use super::*;
use paintcraft_core::types::{
    Rgba,
    picker::{SampleSize, SampleSource},
    resources::SharedBudget,
};
use paintcraft_document::LayerMask;
use paintcraft_raster::coverage_mask::{A8Mask, Limits};
use paintcraft_tools::{Action, ColorPick, PointerButton};
fn setup() -> (egui::Context, WebApp) {
    let ctx = egui::Context::default();
    let mut app = WebApp::new(&ctx);
    let mut output = ctx.run_ui(egui::RawInput::default(), |_| {});
    output.textures_delta.clear();
    let mut document = Editor::new(2, 1).unwrap().session.document().clone();
    let coverage = A8Mask::uniform(
        document.canvas_rect(),
        127,
        &SharedBudget::new(4096),
        Limits::default(),
        &|| Ok(()),
    )
    .unwrap();
    document.layers.get_mut(&document.roots[0]).unwrap().mask =
        Some(LayerMask::new(false, coverage));
    app.editor = Editor::from_document(document).unwrap();
    (ctx, app)
}
#[test]
fn filterless_mask_flatten_uses_full_canvas_transaction_and_undo_restores_intent() {
    let (ctx, mut app) = setup();
    let source = app.editor.session.snapshot();
    let depth = app.editor.session.undo_depth();
    assert!(!render::stored_scoped(&source));
    assert!(app.queue_noise_flatten(&Action::FlattenImage));
    let job = app.filter_display.copy_action.as_mut().unwrap();
    assert_eq!(job.region, source.canvas_rect());
    assert!(matches!(job.source, channel::SourceKey::Appearance(_)));
    let account = job.flatten.as_mut().unwrap().account.take().unwrap();
    job.outcome = Some(Rc::new(RefCell::new(Some(Ok(Output::Flattened(
        flatten::Pixels {
            bytes: vec![17, 31, 47, 255, 9, 19, 29, 127],
            account,
        },
    ))))));
    app.pump_flat_action(&ctx);
    let after = app.editor.session.snapshot();
    assert!(!after.has_layer_masks());
    assert_eq!(
        after.layers[&after.roots[0]].raster.get(1, 0),
        Rgba([9, 19, 29, 127])
    );
    assert_eq!(app.editor.session.undo_depth(), depth + 1);
    assert_eq!(app.clipboard.budget.usage().reserved, 0);
    app.editor.apply(Action::Undo).unwrap();
    assert!(app.editor.session.document().same_semantics(&source));
    app.editor.apply(Action::Redo).unwrap();
    assert!(app.editor.session.document().same_semantics(&after));
}
#[test]
fn filterless_mask_picker_uses_async_image_route_but_raw_layer_stays_legacy() {
    let (_, mut app) = setup();
    app.editor.tools.choose_tool(Tool::ColorPicker);
    let source = app.editor.session.snapshot();
    let mut pick = ColorPick {
        original: app.editor.tools.color,
        revision: app.editor.tools.preview_revision(),
        layer: source.roots[0],
        at: (0, 0),
        button: PointerButton::Primary,
        source: SampleSource::Layer,
        size: SampleSize::One,
        return_resource_shape: None,
        return_to: None,
    };
    assert!(!app.queue_noise_picker(&Action::PickColor(pick.clone())));
    pick.source = SampleSource::Image;
    assert!(app.queue_noise_picker(&Action::PickColor(pick)));
    let job = app.filter_display.picker_wanted.as_ref().unwrap();
    assert_eq!(job.region, Rect::new(0, 0, 1, 1).unwrap());
    assert!(matches!(job.source, channel::SourceKey::Appearance(_)));
    assert!(Arc::ptr_eq(&source, &app.editor.session.snapshot()));
}
#[test]
fn own_mask_print_is_visual_only_and_download_refuses_before_action_adoption() {
    let (ctx, mut app) = setup();
    let source = app.editor.session.snapshot();
    let spec = spec(
        "download",
        "png",
        90,
        [255; 3],
        "studio",
        (false, false),
        &[],
    );
    let result = app.queue_flat_action(
        &ctx,
        source.clone(),
        Purpose::Download {
            name: "x.png".into(),
        },
        source.canvas_rect(),
        (spec, app.empty_copy_mask()),
    );
    assert_eq!(result.unwrap_err().code, ErrorCode::CapabilityUnsupported);
    assert!(app.filter_display.copy_action.is_none());
    app.queue_noise_print(&ctx, source.clone(), "masked".into())
        .unwrap();
    let job = app.filter_display.copy_action.as_ref().unwrap();
    assert_eq!(
        job.spec.get("purpose").and_then(Value::as_str),
        Some("print")
    );
    assert!(matches!(job.source, channel::SourceKey::Appearance(_)));
    assert!(Arc::ptr_eq(&source, &app.editor.session.snapshot()));
}

fn queue_consumer(kind: &str, ctx: &egui::Context, app: &mut WebApp) {
    match kind {
        "print" => app
            .queue_noise_print(ctx, app.editor.session.snapshot(), "captured".into())
            .unwrap(),
        "flatten" => assert!(app.queue_noise_flatten(&Action::FlattenImage)),
        "picker" => {
            app.editor.tools.choose_tool(Tool::ColorPicker);
            let pick = ColorPick {
                original: app.editor.tools.color,
                revision: app.editor.tools.preview_revision(),
                layer: app.editor.session.document().roots[0],
                at: (0, 0),
                button: PointerButton::Primary,
                source: SampleSource::Image,
                size: SampleSize::One,
                return_resource_shape: None,
                return_to: None,
            };
            assert!(app.queue_noise_picker(&Action::PickColor(pick)));
        }
        _ => panic!("unknown test consumer"),
    }
}
fn queued(app: &WebApp) -> &Job {
    app.filter_display
        .copy_action
        .as_ref()
        .or(app.filter_display.picker_wanted.as_ref())
        .unwrap()
}
#[test]
fn actual_queue_captures_some_before_scope_exit_for_all_three_action_constructors() {
    use paintcraft_core::types::resources::AllocationClass;
    use paintcraft_formats::export::layer_appearance::Origin7;
    use paintcraft_render::spatial::{
        WorkspaceAdmission, capture_current_workspace, with_workspace,
    };
    for kind in ["print", "flatten", "picker"] {
        let (ctx, mut app) = setup();
        let source = app.editor.session.snapshot();
        let budget = SharedBudget::new(4096);
        let parent = WorkspaceAdmission::new(budget.scope(AllocationClass::SharedBudget), 513, 200);
        parent.reserve_work(3).unwrap();
        with_workspace(parent.clone(), || {
            queue_consumer(kind, &ctx, &mut app);
            Ok(())
        })
        .unwrap();
        assert!(capture_current_workspace().is_none()); // action may wait across frames
        let job = queued(&app);
        let channel::SourceKey::Appearance(key) = &job.source else {
            panic!("must retain Source7 key")
        };
        assert_eq!(
            key.copy_spec.as_ref(),
            if kind == "print" {
                Some(&job.spec)
            } else {
                None
            }
        );
        // Even an unrelated continuation TLS cannot replace the queued owner.
        let unrelated =
            WorkspaceAdmission::new(budget.scope(AllocationClass::SharedBudget), 4096, 1000);
        let (capture, prefix) =
            with_workspace(unrelated.clone(), || key.prepare_capture(&|| Ok(()))).unwrap();
        assert_eq!(capture.admission.work_authority, Origin7::CapturedJob);
        assert_eq!(capture.admission.workspace_bytes, 513);
        assert_eq!(capture.admission.base_work, 197);
        let used = parent.work_used();
        assert!(used > 3); // real presence/mode scan already debited original parent
        prefix.spend(7).unwrap();
        assert_eq!(parent.work_used(), used + 7);
        assert_eq!(unrelated.work_used(), 0);
        assert!(Arc::ptr_eq(&source, &app.editor.session.snapshot()));
    }
}
#[test]
fn actual_paired_queue_keeps_noise_minimum_and_one_shot_claim_after_tls_exit() {
    use paintcraft_core::types::{
        filters::{AddNoiseRgbV1, Descriptor},
        resources::AllocationClass,
    };
    use paintcraft_document::{FilterInstance, FilterStack};
    use paintcraft_formats::export::layer_appearance::{Mode7, Origin7};
    use paintcraft_render::noise_work::{
        NoiseAdmission, NoiseJobPolicy, NoiseLimits, with_noise_workspace,
    };
    let (ctx, mut app) = setup();
    let mut document = app.editor.session.document().clone();
    document.layers.get_mut(&document.roots[0]).unwrap().filters =
        FilterStack::new(vec![FilterInstance::new(
            Uuid::from_u128(303),
            false,
            Descriptor::AddNoiseRgbV1(AddNoiseRgbV1::default()),
        )])
        .unwrap();
    app.editor = Editor::from_document(document).unwrap();
    let source = app.editor.session.snapshot();
    let budget = SharedBudget::new(16384);
    let parent = NoiseAdmission::new_root(
        budget.scope(AllocationClass::SharedBudget),
        NoiseJobPolicy::BrowserV1,
        NoiseLimits {
            bytes: 4096,
            base_work: 10000,
            noise_work: 7777,
        },
    )
    .unwrap();
    with_noise_workspace(parent.clone(), || {
        queue_consumer("print", &ctx, &mut app);
        Ok(())
    })
    .unwrap();
    let channel::SourceKey::Appearance(key) = &queued(&app).source else {
        panic!("Source7 key")
    };
    let (capture, prefix) = key.prepare_capture(&|| Ok(())).unwrap();
    assert_eq!(capture.admission.work_authority, Origin7::CapturedJob);
    assert_eq!(capture.admission.mode, Mode7::Noise);
    assert_eq!(
        (
            capture.admission.workspace_bytes,
            capture.admission.noise_work
        ),
        (4096, 7777)
    );
    let used = parent.used().unwrap().base;
    prefix.spend(11).unwrap();
    assert_eq!(parent.used().unwrap().base, used + 11);
    let sealed = prefix.seal(3).unwrap();
    assert_eq!(
        (parent.used().unwrap().base, parent.used().unwrap().noise),
        (10000, 7777)
    );
    assert_eq!(sealed.admission.noise_work, 7777);
    assert!(Arc::ptr_eq(&source, &app.editor.session.snapshot()));
    drop(sealed);
}
