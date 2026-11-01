//! Actual background jobs use captured policy on the executing thread.
use super::*;
use paintcraft_core::types::{
    filters::{AddNoiseRgbV1, Descriptor, NoiseDistribution},
    ids::deterministic_id,
    picker::{PickerAfter, SampleSize, SampleSource},
    resources::SharedBudget,
};
use paintcraft_document::{FilterInstance, FilterStack, POLICY};
use paintcraft_tools::PointerButton;
use std::time::{Duration, Instant};
fn setup(distribution: NoiseDistribution) -> (Editor, background::Service, SharedBudget) {
    let mut doc =
        paintcraft_document::new_document(2, 1, Some(Rgba([64, 16, 128, 255])), 0, POLICY).unwrap();
    doc.layers.get_mut(&doc.roots[0]).unwrap().filters =
        FilterStack::new(vec![FilterInstance::new(
            deterministic_id(&["native background Noise"]),
            true,
            Descriptor::AddNoiseRgbV1(AddNoiseRgbV1::new(100, 65535, false, distribution).unwrap()),
        )])
        .unwrap();
    let mut editor = Editor::with_document(doc, None);
    let budget = SharedBudget::new(3 << 30);
    let service =
        background::Service::with_budget(eframe::egui::Context::default(), budget.clone()).unwrap();
    editor.connect_editing(service.clone());
    (editor, service, budget)
}
fn drain(editor: &mut Editor) {
    let deadline = Instant::now() + Duration::from_secs(10);
    while editor.has_pending_edits() {
        editor.run_pending();
        assert!(Instant::now() < deadline, "Noise background task stalled");
        std::thread::yield_now();
    }
}
#[test]
fn background_noise_picker_reads_literal_appearance_without_source_history_or_dirty_change() {
    for (distribution, expected) in [
        (NoiseDistribution::Uniform, [255, 211, 0, 255]),
        (NoiseDistribution::Gaussian, [47, 139, 0, 255]),
    ] {
        let (mut e, _, budget) = setup(distribution);
        let source = e.document_snapshot();
        let ticket = e.session.history_ticket();
        e.options.picker_source = SampleSource::Image;
        e.options.picker_size = SampleSize::One;
        e.options.picker_after = PickerAfter::Stay;
        e.select_tool(Tool::ColorPicker);
        e.tools
            .pointer_down_button(e.target().unwrap(), (0, 0), PointerButton::Primary);
        let (_, Action::PickColor(pick)) =
            e.tools.pointer_up_button(PointerButton::Primary).unwrap()
        else {
            panic!("picker capture")
        };
        e.request(70001, Action::PickColor(pick));
        drain(&mut e);
        assert_eq!(e.color, Rgba(expected));
        assert!(Arc::ptr_eq(&source, &e.document_snapshot()));
        assert_eq!(e.session.history_ticket(), ticket);
        assert_eq!(e.undo_depth(), 0);
        assert!(!e.is_dirty());
        assert_eq!(budget.usage().reserved, 0);
    }
}
#[test]
fn background_noise_flatten_bakes_literal_pixels_and_undo_redo_restores_original_intent() {
    for (distribution, expected) in [
        (
            NoiseDistribution::Uniform,
            [[255, 211, 0, 255], [51, 0, 97, 255]],
        ),
        (
            NoiseDistribution::Gaussian,
            [[47, 139, 0, 255], [0, 4, 46, 255]],
        ),
    ] {
        let (mut e, _, budget) = setup(distribution);
        let before = e.document_snapshot();
        e.request(70002, Action::FlattenImage);
        drain(&mut e);
        assert_eq!(e.undo_depth(), 1);
        assert_eq!(e.document().roots.len(), 1);
        let layer = &e.document().layers[&e.document().roots[0]];
        assert!(layer.filters.is_empty());
        assert_eq!(layer.raster.get(0, 0), Rgba(expected[0]));
        assert_eq!(layer.raster.get(1, 0), Rgba(expected[1]));
        let flattened = e.document_snapshot();
        e.request(70003, Action::Undo);
        drain(&mut e);
        assert!(e.document().same_semantics(&before));
        assert_eq!(
            e.document().layers[&before.roots[0]]
                .raster
                .resident_tiles(),
            0
        );
        e.request(70004, Action::Redo);
        drain(&mut e);
        assert!(e.document().same_semantics(&flattened));
        assert_eq!(budget.usage().reserved, 0);
    }
}
