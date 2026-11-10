//! End-to-end private staging/publication with exact literal pixels and retained metadata.
use paintcraft_commands::{Batch, LayerRef, Mode, Op};
use paintcraft_core::types::{
    Error, ErrorCode, Rect, Rgba, filters::Descriptor, resources::SharedBudget, text::MaskSpan,
};
use paintcraft_document::{Document, FilterInstance, FilterMask, FilterStack, LayerMask, POLICY};
use paintcraft_editor::{execute, session::Session};
use paintcraft_raster::{
    Admission, Selection,
    coverage_mask::{A8Mask, Band, Run},
    selection::{BinaryMask, MaskLimits},
};
use std::cell::Cell;
use uuid::Uuid;
const A: Rgba = Rgba([11, 22, 33, 0]);
const B: Rgba = Rgba([44, 55, 66, 128]);
const C: Rgba = Rgba([77, 88, 99, 255]);
const D: Rgba = Rgba([111, 122, 133, 17]);
fn rect(x: i64, y: i64, w: i64, h: i64) -> Rect {
    Rect::new(x, y, w, h).unwrap()
}
fn scene() -> Document {
    paintcraft_document::import_image(2, 2, &[A.0, B.0, C.0, D.0].concat(), &"12".repeat(32))
        .unwrap()
}
fn batch(session: &Session, operations: Vec<Op>) -> Batch {
    Batch {
        version: 9,
        policy: POLICY.into(),
        base_state: session.ticket().state_id,
        base_epoch: session.ticket().publication_epoch,
        seed: None,
        digest: format!("{:064x}", session.ticket().publication_epoch + 1),
        operations,
    }
}
fn op(width: u32, height: u32, shear: (i32, i32)) -> Op {
    Op::CanvasStretchSkew {
        width,
        height,
        shear,
    }
}
fn assert_pixels(d: &Document, expected: &[Rgba]) {
    let l = &d.layers[&d.roots[0]];
    assert_eq!(
        expected.len(),
        (l.raster.width() * l.raster.height()) as usize
    );
    for y in 0..l.raster.height() {
        for x in 0..l.raster.width() {
            assert_eq!(
                l.raster.get(i64::from(x), i64::from(y)),
                expected[(y * l.raster.width() + x) as usize]
            );
        }
    }
}
#[test]
fn enlargement_keeps_metadata_both_disabled_mask_families_and_one_undo() {
    let mut d = scene();
    let id = d.roots[0];
    d.title = Some("日本語 transform".into());
    let coverage = A8Mask::from_bands(
        rect(0, 0, 2, 2),
        &[Band {
            y0: 0,
            y1: 1,
            runs: &[Run {
                x0: 0,
                x1: 1,
                coverage: 127,
            }],
        }],
        &SharedBudget::new(8192),
        Default::default(),
        &|| Ok(()),
    )
    .unwrap();
    let l = d.layers.get_mut(&id).unwrap();
    l.mask = Some(LayerMask::new(false, coverage.clone()));
    l.filters = FilterStack::new(vec![
        FilterInstance::new(Uuid::from_u128(7), false, Descriptor::InvertRgbV1)
            .with_mask(Some(FilterMask::new(false, coverage))),
    ])
    .unwrap();
    let original = d.clone();
    let mut s = Session::new(d);
    let b = batch(&s, vec![op(4, 4, (0, 0))]);
    assert!(s.apply(&b).unwrap().committed);
    assert_eq!(s.document().canvas, (4, 4));
    assert_eq!(s.document().roots, original.roots);
    assert_eq!(s.document().title, original.title);
    assert_pixels(
        s.document(),
        &[A, A, B, B, A, A, B, B, C, C, D, D, C, C, D, D],
    );
    let l = &s.document().layers[&id];
    assert_eq!(l.name, original.layers[&id].name);
    let own = l.mask.as_ref().unwrap();
    let f = l.filters.as_slice()[0].mask().unwrap();
    assert!(!own.enabled());
    assert!(!f.enabled());
    assert!(!l.filters.as_slice()[0].enabled());
    for field in [own.coverage(), f.coverage()] {
        assert_eq!(field.domain(), rect(0, 0, 4, 4));
        assert_eq!(field.coverage(0, 0), 127);
        assert_eq!(field.coverage(1, 1), 127);
        assert_eq!(field.coverage(2, 0), 0);
    }
    let after = s.snapshot();
    s.undo().unwrap().unwrap();
    assert!(s.document().same_semantics(&original));
    s.redo().unwrap().unwrap();
    assert!(s.document().same_semantics(&after));
}
#[test]
fn shear_padding_and_following_paint_observe_frozen_mapped_lineage() {
    let mut s = Session::new(scene());
    let id = s.document().roots[0];
    let operations = vec![
        op(2, 2, (65536, 0)),
        Op::FillRect {
            layer: LayerRef::Id(id),
            rect: rect(0, 0, 1, 1),
            color: Rgba([3, 4, 5, 6]),
            mode: Mode::Replace,
        },
    ];
    let b = batch(&s, operations);
    s.apply(&b).unwrap();
    assert_eq!(s.document().canvas, (4, 2));
    // Independent pixel-center inverse: x_source=x-y for this unit horizontal shear.
    assert_pixels(
        s.document(),
        &[
            Rgba([3, 4, 5, 6]),
            B,
            Rgba::TRANSPARENT,
            Rgba::TRANSPARENT,
            Rgba::TRANSPARENT,
            C,
            D,
            Rgba::TRANSPARENT,
        ],
    );
}
#[test]
fn selected_reduction_clears_old_samples_and_preserves_unselected_pixels() {
    let mut s = Session::new(scene());
    let id = s.document().roots[0];
    let b = batch(
        &s,
        vec![Op::SelectionSetRect {
            rect: rect(0, 0, 2, 1),
        }],
    );
    s.apply(&b).unwrap();
    let b = batch(
        &s,
        vec![Op::StretchSkewSelection {
            layer: LayerRef::Id(id),
            width: 1,
            height: 1,
            shear: (0, 0),
            clear: Rgba([255; 4]),
        }],
    );
    s.apply(&b).unwrap();
    assert_eq!(s.document().canvas, (2, 2));
    assert_pixels(s.document(), &[B, Rgba([255; 4]), C, D]);
    let Selection::Mask(mask) = s.selection() else {
        panic!("transformed selection");
    };
    assert_eq!(mask.coverage(0, 0), 255);
    assert_eq!(mask.coverage(1, 0), 0);
}
#[test]
fn nonrectangular_selection_maps_holes_and_retains_transformed_offcanvas_pixels() {
    let d = scene();
    let id = d.roots[0];
    let s = Session::new(d);
    let b = batch(
        &s,
        vec![Op::StretchSkewSelection {
            layer: LayerRef::Id(id),
            width: 4,
            height: 2,
            shear: (0, 0),
            clear: Rgba([255; 4]),
        }],
    );
    let mask = BinaryMask::from_spans(
        rect(0, 0, 2, 2),
        false,
        &[
            MaskSpan {
                y: 0,
                x0: 0,
                x1: 0,
                coverage: 255,
            },
            MaskSpan {
                y: 1,
                x0: 1,
                x1: 1,
                coverage: 255,
            },
        ],
        &SharedBudget::new(1 << 20),
        MaskLimits::default(),
        &|| Ok(()),
    )
    .unwrap();
    let out = execute::execute(s.document(), Selection::Mask(mask), &b).unwrap();
    assert_eq!(out.doc.canvas, (2, 2));
    let l = &out.doc.layers[&id];
    assert_eq!(l.raster.width(), 4);
    assert_eq!(l.raster.get(0, 0), A);
    assert_eq!(l.raster.get(1, 0), A);
    assert_eq!(l.raster.get(0, 1), C);
    assert_eq!(l.raster.get(1, 1), Rgba([255; 4]));
    assert_eq!(l.raster.get(2, 1), D);
    assert_eq!(l.raster.get(3, 1), D);
}
#[test]
fn identity_empty_selection_cancel_and_decoded_refusal_never_publish() {
    let mut s = Session::new(scene());
    let before = s.snapshot();
    let ticket = s.history_ticket();
    let b = batch(&s, vec![op(2, 2, (0, 0))]);
    assert!(!s.apply(&b).unwrap().committed);
    assert_eq!(s.history_ticket(), ticket);
    let b = batch(&s, vec![op(4, 4, (0, 0))]);
    for cut in 1..=12 {
        let calls = Cell::new(0);
        let check = || {
            calls.set(calls.get() + 1);
            if calls.get() == cut {
                Err(Error::new(ErrorCode::OperationCancelled, "cut"))
            } else {
                Ok(())
            }
        };
        assert!(
            execute::execute_with_admission_checked(
                s.document(),
                Selection::Inactive,
                &b,
                Admission::new(65536),
                &check
            )
            .is_err()
        );
        assert!(s.document().same_semantics(&before));
        assert_eq!(s.history_ticket(), ticket);
    }
    assert_eq!(
        execute::execute_with_budget(s.document(), Selection::Inactive, &b, 0)
            .unwrap_err()
            .code,
        ErrorCode::ResourceLimit
    );
    let id = s.document().roots[0];
    let b = batch(
        &s,
        vec![Op::StretchSkewSelection {
            layer: LayerRef::Id(id),
            width: 4,
            height: 4,
            shear: (0, 0),
            clear: Rgba::TRANSPARENT,
        }],
    );
    let out = execute::execute(s.document(), Selection::Empty, &b).unwrap();
    assert!(out.doc.same_semantics(&before));
    assert_eq!(out.selection, Selection::Empty);
}
#[test]
fn hidden_offcanvas_layer_extents_ids_blend_and_implicit_defaults_are_not_clipped_or_flattened() {
    let mut d = scene();
    let mut hidden = d.layers[&d.roots[0]].clone();
    hidden.id = Uuid::from_u128(991);
    hidden.origin = (-1, -1);
    hidden.visible = false;
    hidden.opacity = 73;
    hidden.blend_mode = paintcraft_core::types::LayerBlendMode::Multiply;
    hidden.raster = paintcraft_raster::RasterLayer::new(2, 2, Rgba([14, 25, 36, 0]));
    let id = hidden.id;
    d.roots.push(id);
    d.layers.insert(id, hidden);
    let before = d.clone();
    let mut s = Session::new(d);
    let b = batch(&s, vec![op(4, 4, (0, 0))]);
    s.apply(&b).unwrap();
    assert_eq!(s.document().roots, before.roots);
    assert_eq!(s.document().layers.len(), 2);
    let l = &s.document().layers[&id];
    assert_eq!(l.origin, (-2, -2));
    assert_eq!((l.raster.width(), l.raster.height()), (4, 4));
    assert_eq!(l.raster.default_pixel(), Rgba([14, 25, 36, 0]));
    assert_eq!(l.raster.resident_tiles(), 0);
    assert!(!l.visible);
    assert_eq!(l.opacity, 73);
    assert_eq!(
        l.blend_mode,
        paintcraft_core::types::LayerBlendMode::Multiply
    );
}
