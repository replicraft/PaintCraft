//! Public atomic rotation against full RGBA/A8 vectors frozen before consumers.
use paintcraft_commands::{Batch, LayerRef, Op};
use paintcraft_core::types::{
    Error, ErrorCode, Rect, Rgba,
    filters::Descriptor,
    json::{self, Value},
    policy::BlendMode,
    resources::SharedBudget,
};
use paintcraft_document::{Document, FilterInstance, FilterMask, FilterStack, LayerMask, POLICY};
use paintcraft_editor::{execute, session::Session};
use paintcraft_raster::{
    Admission, RasterLayer, Selection,
    coverage_mask::{A8Mask, Band, Run},
    selection::{BinaryMask, MaskLimits},
};
use std::cell::Cell;
use uuid::Uuid;
fn rows(v: &Value) -> &[Value] {
    v.as_array().unwrap()
}
fn get<'a>(v: &'a Value, k: &str) -> &'a Value {
    v.get(k).unwrap()
}
fn ints(v: &Value) -> Vec<i64> {
    rows(v).iter().map(|n| n.as_i64().unwrap()).collect()
}
fn bytes(v: &Value) -> Vec<u8> {
    ints(v)
        .into_iter()
        .map(|n| u8::try_from(n).unwrap())
        .collect()
}
fn rect(v: &Value) -> Rect {
    let a = ints(v);
    Rect {
        x0: a[0],
        y0: a[1],
        x1: a[2],
        y1: a[3],
    }
}
fn pair(v: &Value) -> (i32, i32) {
    let a = ints(v);
    (a[0] as i32, a[1] as i32)
}
fn oracle() -> Value {
    json::parse(include_bytes!(
        "../../../../tests/rotation-q30-fixtures/images.json"
    ))
    .unwrap()
}
fn source(v: &Value) -> (RasterLayer, A8Mask) {
    let sz = ints(get(v, "size"));
    let p = bytes(get(v, "rgba"));
    let d = bytes(get(v, "default"));
    let mut r = RasterLayer::new(sz[0] as u32, sz[1] as u32, Rgba(d.try_into().unwrap()));
    let mut adm = Admission::new(65536);
    for y in 0..sz[1] {
        for x in 0..sz[0] {
            let i = ((y * sz[0] + x) * 4) as usize;
            r.blend_pixel(
                x,
                y,
                Rgba(p[i..i + 4].try_into().unwrap()),
                BlendMode::Replace,
                &mut adm,
            )
            .unwrap();
        }
    }
    let values = bytes(get(v, "mask"));
    let mut runs = Vec::new();
    for (i, c) in values.iter().enumerate() {
        if *c != 0 {
            runs.push((i, *c));
        }
    }
    let band_runs: Vec<Vec<Run>> = (0..sz[1])
        .map(|y| {
            runs.iter()
                .filter(|(i, _)| *i / sz[0] as usize == y as usize)
                .map(|(i, c)| Run {
                    x0: (*i % sz[0] as usize) as i64,
                    x1: (*i % sz[0] as usize) as i64 + 1,
                    coverage: *c,
                })
                .collect()
        })
        .collect();
    let bands: Vec<Band<'_>> = band_runs
        .iter()
        .enumerate()
        .filter(|(_, r)| !r.is_empty())
        .map(|(y, r)| Band {
            y0: y as i64,
            y1: y as i64 + 1,
            runs: r,
        })
        .collect();
    let mask = A8Mask::from_bands(
        Rect::new(0, 0, sz[0], sz[1]).unwrap(),
        &bands,
        &SharedBudget::new(1 << 20),
        Default::default(),
        &|| Ok(()),
    )
    .unwrap();
    (r, mask)
}
fn scene(case: &Value, whole: bool) -> Document {
    let mut d = paintcraft_document::import_image(4, 3, &[0u8; 48], &"12".repeat(32)).unwrap();
    let template = d.layers[&d.roots[0]].clone();
    d.layers.clear();
    d.roots.clear();
    let inputs: Vec<&Value> = if whole {
        rows(get(case, "layers"))
            .iter()
            .map(|l| get(l, "source"))
            .collect()
    } else {
        vec![get(case, "source")]
    };
    for (i, v) in inputs.into_iter().enumerate() {
        let mut l = template.clone();
        l.id = Uuid::from_u128(100 + i as u128);
        let o = ints(get(v, "origin"));
        l.origin = (o[0], o[1]);
        let (r, m) = source(v);
        l.raster = r;
        l.mask = Some(LayerMask::new(false, m.clone()));
        l.filters = FilterStack::new(vec![
            FilterInstance::new(Uuid::from_u128(700), false, Descriptor::InvertRgbV1)
                .with_mask(Some(FilterMask::new(false, m))),
        ])
        .unwrap();
        l.visible = !get(v, "hidden").as_bool().unwrap();
        l.opacity = 73 + i as u8;
        l.blend_mode = paintcraft_core::types::LayerBlendMode::Multiply;
        d.roots.push(l.id);
        d.layers.insert(l.id, l);
    }
    d
}
fn batch(s: &Session, operations: Vec<Op>) -> Batch {
    Batch {
        version: 11,
        policy: POLICY.into(),
        base_state: s.ticket().state_id,
        base_epoch: s.ticket().publication_epoch,
        seed: None,
        digest: format!("{:064x}", s.ticket().publication_epoch + 1),
        operations,
    }
}
fn rgba(r: &RasterLayer) -> Vec<u8> {
    let mut out = Vec::new();
    for y in 0..r.height() {
        for x in 0..r.width() {
            out.extend(r.get(i64::from(x), i64::from(y)).0)
        }
    }
    out
}
fn coverage(m: &A8Mask, w: u32, h: u32) -> Vec<u8> {
    (0..h)
        .flat_map(|y| (0..w).map(move |x| m.coverage(i64::from(x), i64::from(y))))
        .collect()
}
#[test]
fn all_layer_common_pivot_hidden_offcanvas_rgba_both_masks_and_single_undo() {
    for case in rows(get(&oracle(), "whole")) {
        let original = scene(case, true);
        let mut s = Session::new(original.clone());
        let b = batch(
            &s,
            vec![Op::CanvasRotate {
                coefficients: pair(get(case, "coefficients")),
            }],
        );
        assert!(s.apply(&b).unwrap().committed);
        let cb = rect(get(case, "canvas_bounds"));
        assert_eq!(s.document().canvas, (cb.width() as u32, cb.height() as u32));
        assert_eq!(s.document().roots, original.roots);
        for (i, expected) in rows(get(case, "layers")).iter().enumerate() {
            let id = original.roots[i];
            let l = &s.document().layers[&id];
            let dest = rect(get(expected, "destination"));
            assert_eq!(l.extent(), dest);
            assert_eq!(rgba(&l.raster), bytes(get(expected, "rgba")));
            assert_eq!(l.visible, original.layers[&id].visible);
            assert_eq!(l.opacity, original.layers[&id].opacity);
            assert_eq!(l.blend_mode, original.layers[&id].blend_mode);
            let own = l.mask.as_ref().unwrap();
            let filter = l.filters.as_slice()[0].mask().unwrap();
            assert!(!own.enabled());
            assert!(!filter.enabled());
            assert!(!l.filters.as_slice()[0].enabled());
            for m in [own.coverage(), filter.coverage()] {
                assert_eq!(
                    coverage(m, l.raster.width(), l.raster.height()),
                    bytes(get(expected, "mask"))
                );
            }
        }
        let after = s.snapshot();
        s.undo().unwrap().unwrap();
        assert!(s.document().same_semantics(&original));
        s.redo().unwrap().unwrap();
        assert!(s.document().same_semantics(&after));
    }
}
#[test]
fn selected_holes_clear_replace_and_world_field_masks_match_frozen_vectors() {
    for case in rows(get(&oracle(), "selected")) {
        let d = scene(case, false);
        let s = Session::new(d);
        let id = s.document().roots[0];
        let mut spans: Vec<paintcraft_core::types::text::MaskSpan> =
            rows(get(case, "selected_points"))
                .iter()
                .map(|p| {
                    let p = ints(p);
                    paintcraft_core::types::text::MaskSpan {
                        y: p[1],
                        x0: p[0],
                        x1: p[0],
                        coverage: 255,
                    }
                })
                .collect();
        spans.sort_by_key(|span| (span.y, span.x0));
        let mask = BinaryMask::from_spans(
            rect(get(case, "selection_area")),
            false,
            &spans,
            &SharedBudget::new(1 << 20),
            MaskLimits::default(),
            &|| Ok(()),
        )
        .unwrap();
        let clear = Rgba(bytes(get(case, "clear")).try_into().unwrap());
        let b = batch(
            &s,
            vec![Op::RotateSelection {
                layer: LayerRef::Id(id),
                coefficients: pair(get(case, "coefficients")),
                clear,
            }],
        );
        let out = execute::execute(s.document(), Selection::Mask(mask), &b).unwrap();
        let l = &out.doc.layers[&id];
        assert_eq!(l.extent(), rect(get(case, "destination")));
        assert_eq!(rgba(&l.raster), bytes(get(case, "rgba")));
        let Selection::Mask(mask) = &out.selection else {
            panic!("mapped mask")
        };
        let mut selected = Vec::new();
        for y in 0..3 {
            for x in 0..4 {
                selected.push(mask.coverage(x, y));
            }
        }
        assert_eq!(selected, bytes(get(case, "selection")));
        assert_eq!(
            coverage(
                l.mask.as_ref().unwrap().coverage(),
                l.raster.width(),
                l.raster.height()
            ),
            bytes(get(case, "rebased_layer_mask"))
        );
        assert_eq!(
            coverage(
                l.filters.as_slice()[0].mask().unwrap().coverage(),
                l.raster.width(),
                l.raster.height()
            ),
            bytes(get(case, "rebased_layer_mask"))
        );
    }
}
#[test]
fn identity_empty_cancel_exhaustion_and_following_paint_do_not_escape_atomic_staging() {
    let o = oracle();
    let mut s = Session::new(scene(&rows(get(&o, "whole"))[3], true));
    let before = s.snapshot();
    let history = s.history_ticket();
    let b = batch(
        &s,
        vec![Op::CanvasRotate {
            coefficients: (1 << 30, 0),
        }],
    );
    assert!(!s.apply(&b).unwrap().committed);
    assert_eq!(s.history_ticket(), history);
    let b = batch(
        &s,
        vec![Op::CanvasRotate {
            coefficients: (644245094, 858993459),
        }],
    );
    for cut in 1..=16 {
        let calls = Cell::new(0);
        let check = || {
            calls.set(calls.get() + 1);
            if calls.get() == cut {
                Err(Error::new(ErrorCode::OperationCancelled, "original cut"))
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
        assert_eq!(s.history_ticket(), history);
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
        vec![Op::RotateSelection {
            layer: LayerRef::Id(id),
            coefficients: (644245094, 858993459),
            clear: Rgba::TRANSPARENT,
        }],
    );
    let out = execute::execute(s.document(), Selection::Empty, &b).unwrap();
    assert!(out.doc.same_semantics(&before));
    assert_eq!(out.selection, Selection::Empty);
    let b = batch(
        &s,
        vec![
            Op::CanvasRotate {
                coefficients: (644245094, 858993459),
            },
            Op::FillRect {
                layer: LayerRef::Id(id),
                rect: Rect::new(0, 0, 1, 1).unwrap(),
                color: Rgba([3, 4, 5, 6]),
                mode: paintcraft_commands::Mode::Replace,
            },
        ],
    );
    s.apply(&b).unwrap();
    let l = &s.document().layers[&id];
    assert_eq!(l.raster.get(-l.origin.0, -l.origin.1), Rgba([3, 4, 5, 6]));
    s.undo().unwrap();
    assert!(s.document().same_semantics(&before));
}

#[test]
fn stored_noise_default_and_disabled_refuse_original_geometry_guard() {
    use paintcraft_core::types::filters::{AddNoiseRgbV1, NoiseDistribution};
    let o = oracle();
    for (enabled, amount) in [(true, 0), (false, 0), (true, 10), (false, 10)] {
        let mut d = scene(&rows(get(&o, "whole"))[3], true);
        let id = d.roots[0];
        d.layers.get_mut(&id).unwrap().filters = FilterStack::new(vec![FilterInstance::new(
            Uuid::from_u128(99),
            enabled,
            Descriptor::AddNoiseRgbV1(
                AddNoiseRgbV1::new(amount, 1, true, NoiseDistribution::Uniform).unwrap(),
            ),
        )])
        .unwrap();
        let mut session = Session::new(d);
        let before = session.snapshot();
        let ticket = session.history_ticket();
        for op in [
            Op::CanvasRotate {
                coefficients: (644245094, 858993459),
            },
            Op::CanvasRotate {
                coefficients: (0, 1 << 30),
            },
            Op::RotateSelection {
                layer: LayerRef::Id(id),
                coefficients: (644245094, 858993459),
                clear: Rgba::TRANSPARENT,
            },
        ] {
            let b = batch(&session, vec![op]);
            // Establish an active selection so selected geometry reaches the stored-noise guard.
            let selection = Selection::Rect(Rect::new(0, 0, 2, 2).unwrap());
            assert_eq!(
                execute::execute(session.document(), selection, &b)
                    .unwrap_err()
                    .code,
                ErrorCode::CapabilityUnsupported
            );
            assert!(session.document().same_semantics(&before));
            assert_eq!(session.history_ticket(), ticket);
        }
        let b = batch(
            &session,
            vec![Op::CanvasRotate {
                coefficients: (644245094, 858993459),
            }],
        );
        assert!(session.apply(&b).is_err());
    }
}
