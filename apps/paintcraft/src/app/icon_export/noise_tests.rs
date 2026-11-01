//! Actual background asset preparation/encoding; no OS dialog is opened.
use super::*;
use paintcraft_core::types::{
    filters::{AddNoiseRgbV1, Descriptor, NoiseDistribution},
    ids::deterministic_id,
};
use paintcraft_document::{FilterInstance, FilterStack};
use std::time::Instant;
fn app() -> App {
    let mut doc = paintcraft_document::new_document(
        32,
        32,
        Some(Rgba([64, 16, 128, 255])),
        0,
        paintcraft_document::POLICY,
    )
    .unwrap();
    doc.layers.get_mut(&doc.roots[0]).unwrap().filters =
        FilterStack::new(vec![FilterInstance::new(
            deterministic_id(&["asset Noise"]),
            true,
            Descriptor::AddNoiseRgbV1(
                AddNoiseRgbV1::new(100, 65535, false, NoiseDistribution::Uniform).unwrap(),
            ),
        )])
        .unwrap();
    let mut app = App::new(None);
    app.settings.experience = Experience::Studio;
    app.editor = Editor::with_document(doc, None);
    app.begin_icon_export();
    app
}
#[test]
fn actual_noise_icon_export_prepare_encode_and_escaped_cache_preserve_literal_source() {
    for kind in [widget::Kind::Icon, widget::Kind::Cursor] {
        let mut app = app();
        let before = app.editor.document_snapshot();
        let ticket = app.editor.history_view().ticket;
        let ctx = app.context.clone();
        let budget = paintcraft_runtime::SharedBudget::new(1 << 30);
        let mut state = app.icon_export.take().unwrap();
        state.draft.config.kind = kind;
        state.draft.config.filter = widget::Filter::Nearest;
        state.draft.config.entries = vec![widget::Entry {
            width: 32,
            height: 32,
            encoding: widget::Encoding::Png,
            hotspot: (3, 4),
        }];
        state.prepare(&ctx, &budget);
        assert!(state.cache.value().is_none());
        assert_eq!(budget.usage().reserved, 0, "bake not authorized");
        state.copy.bake = true;
        let deadline = Instant::now() + Duration::from_secs(10);
        while state.cache.value().is_none() {
            state.prepare(&ctx, &budget);
            assert!(Instant::now() < deadline);
            std::thread::yield_now();
        }
        let escaped = state.cache.value().unwrap().clone();
        assert_eq!(
            &escaped.entries()[0].rgba()[..8],
            &[255, 211, 0, 255, 51, 0, 97, 255]
        );
        assert!(
            state.bytes.is_none(),
            "preview alone never authorizes encoding"
        );
        let losses = other_losses(escaped.losses());
        state.background.request_encode(&losses).unwrap();
        while state.bytes.is_none() {
            state.prepare(&ctx, &budget);
            assert!(Instant::now() < deadline);
            std::thread::yield_now();
        }
        let bytes = state.bytes.take().unwrap();
        let decoded = icons::decode_entry(&bytes, 0, 1024).unwrap().rgba;
        assert_eq!(&decoded[..8], &[255, 211, 0, 255, 51, 0, 97, 255]);
        assert!(Arc::ptr_eq(&before, &app.editor.document_snapshot()));
        assert_eq!(ticket, app.editor.history_view().ticket);
        assert!(!app.editor.is_dirty());
        assert_eq!(before.layers[&before.roots[0]].raster.resident_tiles(), 0);
        drop(bytes);
        app.editor = Editor::with_document(
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
        assert!(
            !state.current(&app),
            "replacement retires this private asset intent"
        );
        drop(state);
        drop(app);
        assert!(
            budget.usage().reserved > 0,
            "escaped Prepared owns retention after cancel/source destruction"
        );
        assert_eq!(escaped.entries()[0].rgba()[0], 255);
        drop(escaped);
        assert_eq!(budget.usage().reserved, 0);
    }
}
