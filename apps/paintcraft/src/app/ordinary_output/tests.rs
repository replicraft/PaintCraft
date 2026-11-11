//! Actual dedicated preparation threads; no native printer/clipboard dialogs.
use super::*;
use paintcraft_core::types::{
    filters::{AddNoiseRgbV1, Descriptor, NoiseDistribution},
    ids::deterministic_id,
};
use paintcraft_document::{FilterInstance, FilterStack};
use std::time::Instant;
fn app(distribution: NoiseDistribution) -> App {
    let mut doc = paintcraft_document::new_document(
        2,
        1,
        Some(Rgba([64, 16, 128, 255])),
        0,
        paintcraft_document::POLICY,
    )
    .unwrap();
    doc.layers.get_mut(&doc.roots[0]).unwrap().filters =
        FilterStack::new(vec![FilterInstance::new(
            deterministic_id(&["ordinary output Noise"]),
            true,
            Descriptor::AddNoiseRgbV1(AddNoiseRgbV1::new(100, 65535, false, distribution).unwrap()),
        )])
        .unwrap();
    let mut a = App::new(None);
    a.settings.experience = Experience::Studio;
    a.editor = Editor::with_document(doc, None);
    a
}
fn pixels(distribution: NoiseDistribution) -> [u8; 8] {
    match distribution {
        NoiseDistribution::Uniform => [255, 211, 0, 255, 51, 0, 97, 255],
        NoiseDistribution::Gaussian => [47, 139, 0, 255, 0, 4, 46, 255],
    }
}
fn collect(state: &mut State) -> Result<Packet> {
    let deadline = Instant::now() + Duration::from_secs(10);
    loop {
        assert!(Instant::now() < deadline, "output worker stalled");
        match state.pending.as_ref().unwrap().receiver.try_recv() {
            Ok(Event::Complete(result)) => {
                state.pending = None;
                return result;
            }
            Ok(Event::Ready) => panic!("unexpected publication request"),
            Err(mpsc::TryRecvError::Empty) => std::thread::yield_now(),
            Err(mpsc::TryRecvError::Disconnected) => panic!("worker disconnected"),
        }
    }
}
fn drain(a: &mut App) {
    let ctx = a.context.clone();
    let deadline = Instant::now() + Duration::from_secs(10);
    while a.ordinary_output.busy() {
        a.poll_ordinary_output(&ctx);
        assert!(Instant::now() < deadline, "UI output coordinator stalled");
        std::thread::yield_now();
    }
}
fn policy(frozen: &Frozen) -> export::metadata_policy::Policy {
    export::metadata_policy::Policy::capture(
        &frozen.document,
        export::metadata_policy::Target::new(
            paintcraft_formats::registry::Family::Png,
            paintcraft_formats::registry::Operation::ExportFlat,
        ),
        export::metadata_policy::Intent::Refuse,
    )
    .unwrap()
}
fn baked(a: &App) -> Frozen {
    let mut f = Frozen::capture(a).unwrap();
    f.filter_bake = export::filter_bake::Policy::capture(
        &f.document,
        export::metadata_policy::Target::new(
            paintcraft_formats::registry::Family::Png,
            paintcraft_formats::registry::Operation::ExportFlat,
        ),
        export::filter_bake::Intent::Bake,
        &|| Ok(()),
    )
    .unwrap();
    f
}
#[test]
fn background_print_png_uses_literal_noise_and_retains_output_until_consumer_drop() {
    for distribution in [NoiseDistribution::Uniform, NoiseDistribution::Gaussian] {
        let a = app(distribution);
        let source = a.editor.document_snapshot();
        let ticket = a.editor.history_view().ticket;
        let budget = paintcraft_runtime::SharedBudget::new(256 << 20);
        let mut state = State::default();
        state
            .start(
                Frozen::capture(&a).unwrap(),
                Intent::Print("Captured title".into()),
                &budget,
                &a.context,
            )
            .unwrap();
        let packet = collect(&mut state).unwrap();
        assert!(budget.usage().reserved > 0);
        let Output::Flat(flat) = &packet.output else {
            panic!("print PNG");
        };
        assert_eq!(
            image::decode(&flat.bytes).unwrap().rgba,
            pixels(distribution)
        );
        assert!(Arc::ptr_eq(&source, &a.editor.document_snapshot()));
        assert_eq!(ticket, a.editor.history_view().ticket);
        assert!(!a.editor.is_dirty());
        assert_eq!(source.layers[&source.roots[0]].raster.resident_tiles(), 0);
        drop(packet);
        assert_eq!(budget.usage().reserved, 0);
    }
}
#[test]
fn background_export_preserves_bake_refusal_then_plans_and_writes_exact_png() {
    let mut a = app(NoiseDistribution::Gaussian);
    let source = a.editor.document_snapshot();
    let ticket = a.editor.history_view().ticket;
    let path = std::env::temp_dir().join(format!(
        "paintcraft-noise-background-{}.png",
        Uuid::new_v4()
    ));
    a.plan_export_frozen(
        path.clone(),
        Settings::default(),
        Frozen::capture(&a).unwrap(),
    );
    drain(&mut a);
    assert!(a.confirm.is_none());
    assert!(!path.exists());
    a.plan_export_frozen(path.clone(), Settings::default(), baked(&a));
    drain(&mut a);
    let Some(Confirm::Export {
        format,
        losses,
        frozen,
        metadata_policy,
        ..
    }) = a.confirm.take()
    else {
        panic!("actual loss review");
    };
    assert!(
        losses
            .iter()
            .any(|s| s == export::filter_bake::BAKED_REASON)
    );
    a.do_export_frozen(
        &path,
        format,
        &losses,
        Settings::default(),
        *frozen,
        metadata_policy,
    );
    drain(&mut a);
    assert_eq!(
        image::decode(&std::fs::read(&path).unwrap()).unwrap().rgba,
        pixels(NoiseDistribution::Gaussian)
    );
    assert!(Arc::ptr_eq(&source, &a.editor.document_snapshot()));
    assert_eq!(ticket, a.editor.history_view().ticket);
    assert!(!a.editor.is_dirty());
    assert_eq!(a.worker_budget.usage().reserved, 0);
    std::fs::remove_file(path).unwrap();
}
#[test]
fn private_ready_bytes_cannot_publish_after_source_replacement_and_cancel_releases() {
    let mut a = app(NoiseDistribution::Uniform);
    let f = baked(&a);
    let metadata = policy(&f);
    let path = std::env::temp_dir().join(format!("paintcraft-noise-stale-{}.png", Uuid::new_v4()));
    a.start_ordinary_output(
        f,
        Intent::Save {
            path: path.clone(),
            format: ImageFormat::Png,
            settings: Settings::default(),
            policy: metadata,
            losses: KNOWN_LOSSES.iter().map(|s| (*s).to_owned()).collect(),
        },
    );
    // Let the actual encoder reach the authorization boundary without authorizing
    // a pathname. The only successful write route still belongs to UI adoption.
    let pending = a.ordinary_output.pending.as_ref().unwrap();
    assert!(matches!(
        pending
            .receiver
            .recv_timeout(Duration::from_secs(10))
            .unwrap(),
        Event::Ready
    ));
    assert!(!path.exists());
    a.editor = Editor::with_document(
        paintcraft_document::new_document(
            1,
            1,
            Some(Rgba([1, 2, 3, 255])),
            0,
            paintcraft_document::POLICY,
        )
        .unwrap(),
        None,
    );
    drain(&mut a);
    assert!(!path.exists());
    assert_eq!(a.worker_budget.usage().reserved, 0);
}
#[test]
fn noise_pcx_preparation_is_background_and_literal_preview_keeps_its_lease() {
    let a = app(NoiseDistribution::Uniform);
    let budget = paintcraft_runtime::SharedBudget::new(256 << 20);
    let mut state = State::default();
    state
        .start(
            Frozen::capture(&a).unwrap(),
            Intent::Pcx(paintcraft_ui::pcx_export::Encoding::Rgb24),
            &budget,
            &a.context,
        )
        .unwrap();
    let packet = collect(&mut state).unwrap();
    let Output::Preview(p) = &packet.output else {
        panic!("PCX preview");
    };
    assert_eq!((p.width, p.height), (2, 1));
    assert_eq!(p.rgba, pixels(NoiseDistribution::Uniform));
    assert!(!p.quantized);
    assert!(budget.usage().reserved > 0);
    drop(packet);
    assert_eq!(budget.usage().reserved, 0);
}

#[test]
fn actual_native_print_png_preparation_adopts_blends_without_foreign_bake_authority() {
    use crate::files::blend_adoption_tests::{expected, source as blend_source};
    use paintcraft_core::types::LayerBlendMode;
    for mode in [LayerBlendMode::Multiply, LayerBlendMode::Screen] {
        for decorated in [false, true] {
            let doc = blend_source(mode, 2, 1, decorated);
            let mut a = App::new(None);
            a.install(Editor::with_document(doc.clone(), None)).unwrap();
            let source = a.editor.document_snapshot();
            let ticket = a.editor.history_view().ticket;
            let budget = paintcraft_runtime::SharedBudget::new(256 << 20);
            let mut state = State::default();
            state
                .start(
                    Frozen::capture(&a).unwrap(),
                    Intent::Print("Blend PNG only".into()),
                    &budget,
                    &a.context,
                )
                .unwrap();
            let packet = collect(&mut state).unwrap();
            assert!(budget.usage().reserved > 0);
            let Output::Flat(flat) = &packet.output else {
                panic!("print PNG only")
            };
            let decoded = image::decode(&flat.bytes).unwrap();
            assert_eq!((decoded.width, decoded.height), (2, 1));
            assert_eq!(decoded.rgba, expected(mode, decorated).repeat(2));
            assert!(Arc::ptr_eq(&source, &a.editor.document_snapshot()));
            assert!(a.editor.document().same_semantics(&doc));
            assert_eq!(a.editor.history_view().ticket, ticket);
            drop(packet);
            assert_eq!(budget.usage().reserved, 0);
        }
    }
}
