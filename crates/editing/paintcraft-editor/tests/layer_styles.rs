use paintcraft_commands::{Batch, LayerRef, StyleOp, StyleRef};
use paintcraft_core::types::{
    ErrorCode, Rgba,
    json::Value,
    resources::{AllocationClass, SharedBudget},
    styles::{ColorOverlayRgbV1, EffectBlendMode, StyleDescriptor},
};
use paintcraft_document::{Document, StyleInstance, StyleStack};
use paintcraft_editor::{execute, session::Session};
use paintcraft_raster::Admission;
use std::sync::Arc;
use uuid::Uuid;
fn document() -> Document {
    paintcraft_document::new_document(
        1000,
        486,
        Some(Rgba([7, 9, 11, 0])),
        0,
        paintcraft_document::POLICY,
    )
    .unwrap()
}
fn descriptor(color: [u8; 3]) -> StyleDescriptor {
    StyleDescriptor::ColorOverlayRgbV1(ColorOverlayRgbV1::new(color, 0, EffectBlendMode::Screen))
}
fn batch(doc: &Document, epoch: u64, operations: Vec<Value>) -> Batch {
    Batch::from_generated_value(Value::object([
        (
            "schema",
            Value::str(Batch::schema_for_operations(&operations)),
        ),
        ("atomic", Value::Bool(true)),
        ("policy", Value::str(paintcraft_document::POLICY)),
        (
            "base",
            Value::object([
                ("state_id", Value::str(doc.state_id.to_string())),
                ("publication_epoch", Value::str(epoch.to_string())),
            ]),
        ),
        ("operations", Value::Array(operations)),
    ]))
    .unwrap()
}
fn apply(s: &mut Session, ops: Vec<StyleOp>) -> paintcraft_editor::session::Receipt {
    let packet = batch(
        s.document(),
        s.ticket().publication_epoch,
        ops.iter().map(|op| op.to_value().unwrap()).collect(),
    );
    s.apply(&packet).unwrap()
}
fn add(layer: Uuid, alias: &str) -> StyleOp {
    StyleOp::Add {
        layer: LayerRef::Id(layer),
        alias: alias.into(),
        descriptor: descriptor([13, 71, 201]),
        enabled: false,
        to_index: None,
    }
}
#[test]
fn styles_real_atomic_aliases_scoped_duplication_history_and_identity_noops() {
    let mut a = Session::new(document());
    let mut b = Session::new(document());
    let layer = a.document().roots[0];
    let source = a.snapshot();
    let receipt = apply(&mut a, vec![add(layer, "first")]);
    assert_eq!(receipt, apply(&mut b, vec![add(layer, "first")]));
    assert_eq!(
        receipt.to_value().get("schema").unwrap().as_str(),
        Some("paintcraft.result/layer-styles-v1")
    );
    let id = receipt.style_aliases["first"].1;
    assert!(!id.is_nil());
    assert!(receipt.filter_aliases.is_empty());
    let before = a.snapshot();
    let ticket = a.ticket();
    let history = a.history_view();
    for operation in [
        StyleOp::Update {
            layer: LayerRef::Id(layer),
            style: StyleRef::Id(id),
            descriptor: descriptor([13, 71, 201]),
        },
        StyleOp::SetEnabled {
            layer: LayerRef::Id(layer),
            style: StyleRef::Id(id),
            enabled: false,
        },
        StyleOp::Move {
            layer: LayerRef::Id(layer),
            style: StyleRef::Id(id),
            to_index: 0,
        },
    ] {
        let r = apply(&mut a, vec![operation]);
        assert!(!r.committed);
        assert!(r.style_edit);
        assert!(r.to_value().get("style_aliases").is_some());
        assert!(Arc::ptr_eq(&before, &a.snapshot()));
        assert_eq!(a.ticket(), ticket);
        assert_eq!(a.history_view(), history);
    }
    // Opacity0 RGB is still complete semantic intent, not a pixel no-op.
    assert!(
        apply(
            &mut a,
            vec![StyleOp::Update {
                layer: LayerRef::Id(layer),
                style: StyleRef::Id(id),
                descriptor: descriptor([1, 2, 3])
            }]
        )
        .committed
    );
    let copied = apply(
        &mut a,
        vec![StyleOp::Duplicate {
            layer: LayerRef::Id(layer),
            style: StyleRef::Id(id),
            alias: "copy".into(),
            to_index: Some(0),
        }],
    );
    assert_ne!(copied.style_aliases["copy"].1, id);
    let changed = a.snapshot();
    a.undo().unwrap();
    a.redo().unwrap();
    assert!(a.document().same_semantics(&changed));
    assert_eq!(
        a.document().layers[&layer].raster,
        source.layers[&layer].raster
    );
    // Layer duplicate carries style IDs into another layer scope.
    let op = Value::object([
        ("op", Value::str("layer.duplicate")),
        (
            "layer_ref",
            Value::object([("id", Value::str(layer.to_string()))]),
        ),
        ("ref", Value::str("layercopy")),
        ("name", Value::str("Copy")),
    ]);
    let p = batch(a.document(), a.ticket().publication_epoch, vec![op]);
    let copied = a.apply(&p).unwrap();
    let other = copied.aliases["layercopy"];
    assert_eq!(
        a.document().layers[&other].styles,
        a.document().layers[&layer].styles
    );
}
#[test]
fn max256_defaultcanvas_original_scope_exact_short_and_compact_preview_preserve_source() {
    let mut doc = document();
    let layer = doc.roots[0];
    doc.layers.get_mut(&layer).unwrap().styles = StyleStack::new(
        (1..=256)
            .map(|n| StyleInstance::new(Uuid::from_u128(n), false, descriptor([1, 2, 3])).unwrap())
            .collect(),
    )
    .unwrap();
    let op = StyleOp::Update {
        layer: LayerRef::Id(layer),
        style: StyleRef::Id(Uuid::from_u128(1)),
        descriptor: descriptor([3, 2, 1]),
    };
    let p = batch(&doc, 0, vec![op.to_value().unwrap()]);
    let quote = execute::metadata_clone::quote_checked(&doc, &p, &|| Ok(()), &|_| Ok(())).unwrap();
    assert!(quote.peak_bytes < 1 << 20);
    let original = SharedBudget::new(1 << 20);
    let scope = original
        .reserve_capacity(1 << 20)
        .unwrap()
        .scope(AllocationClass::PaintMetadata);
    let adm = Admission::new(0).with_paint_metadata_scope(scope.clone());
    let staged =
        execute::metadata_clone::stage_checked(&doc, &p, adm, &scope, &|| Ok(()), &|_| Ok(()))
            .unwrap();
    assert_eq!(staged.document().layers[&layer].styles.len(), 256);
    assert_eq!(
        staged.document().layers[&layer].raster,
        doc.layers[&layer].raster
    );
    assert_eq!(
        doc.layers[&layer].styles.as_slice()[0].descriptor(),
        descriptor([1, 2, 3])
    );
    assert!(scope.footprint().live > 0);
    drop(staged);
    assert_eq!(scope.footprint().live, 0);
    for cap in [quote.peak_bytes - 1, quote.peak_bytes] {
        let budget = SharedBudget::new(cap);
        let scope = budget
            .reserve_capacity(cap)
            .unwrap()
            .scope(AllocationClass::PaintMetadata);
        let result = execute::metadata_clone::stage_checked(
            &doc,
            &p,
            Admission::new(0).with_paint_metadata_scope(scope.clone()),
            &scope,
            &|| Ok(()),
            &|_| Ok(()),
        );
        assert_eq!(result.is_ok(), cap == quote.peak_bytes);
        drop(result);
        assert_eq!(scope.footprint().live, 0);
    }
    let p = batch(&doc, 0, vec![add(layer, "excess").to_value().unwrap()]);
    assert_eq!(
        execute::execute(&doc, execute::Selection::Inactive, &p)
            .unwrap_err()
            .code,
        ErrorCode::ResourceLimit
    );
}
#[test]
fn styled_base_ordinary_operation_cannot_bypass_original_style_clone_admission() {
    let mut doc = document();
    let layer = doc.roots[0];
    doc.layers.get_mut(&layer).unwrap().styles = StyleStack::new(vec![
        StyleInstance::new(Uuid::from_u128(1), false, descriptor([1, 2, 3])).unwrap(),
    ])
    .unwrap();
    let p = batch(
        &doc,
        0,
        vec![Value::object([
            ("op", Value::str("layer.set_properties")),
            (
                "layer_ref",
                Value::object([("id", Value::str(layer.to_string()))]),
            ),
            ("name", Value::str("Changed")),
        ])],
    );
    let q = execute::style_controls::quote_checked(&doc, &p, &|| Ok(())).unwrap();
    assert!(q.bytes > 0);
    let budget = SharedBudget::new(q.bytes - 1);
    let scope = budget
        .reserve_capacity(q.bytes - 1)
        .unwrap()
        .scope(AllocationClass::PaintMetadata);
    let error = execute::execute_with_admission(
        &doc,
        execute::Selection::Inactive,
        &p,
        Admission::new(0).with_paint_metadata_scope(scope.clone()),
    )
    .unwrap_err();
    assert_eq!(error.code, ErrorCode::ResourceLimit);
    assert_eq!(scope.footprint().live, 0);
    let plain = document();
    let p = batch(
        &plain,
        0,
        vec![Value::object([("op", Value::str("selection.clear"))])],
    );
    assert_eq!(
        execute::style_controls::quote_checked(&plain, &p, &|| Ok(()))
            .unwrap()
            .bytes,
        0
    );
}
#[test]
fn stale_preparation_and_late_failure_keep_original_doc_history_and_style_intent() {
    let mut s = Session::new(document());
    let layer = s.document().roots[0];
    let before = s.snapshot();
    let ticket = s.ticket();
    let operations = vec![
        add(layer, "first").to_value().unwrap(),
        StyleOp::Remove {
            layer: LayerRef::Id(layer),
            style: StyleRef::Id(Uuid::from_u128(9999)),
        }
        .to_value()
        .unwrap(),
    ];
    let p = batch(s.document(), s.ticket().publication_epoch, operations);
    assert!(s.apply(&p).is_err());
    assert!(Arc::ptr_eq(&before, &s.snapshot()));
    assert_eq!(s.ticket(), ticket);
    assert_eq!(s.undo_depth(), 0);
    let p = batch(
        s.document(),
        s.ticket().publication_epoch,
        vec![add(layer, "first").to_value().unwrap()],
    );
    let prep = s.preparation().unwrap();
    let prepared = prep.apply(&p, || Ok(())).unwrap();
    let clear = batch(
        s.document(),
        s.ticket().publication_epoch,
        vec![Value::object([
            ("op", Value::str("selection.set_rect")),
            (
                "rect",
                Value::object([
                    ("x", Value::Int(0)),
                    ("y", Value::Int(0)),
                    ("width", Value::Int(1)),
                    ("height", Value::Int(1)),
                ]),
            ),
        ])],
    );
    s.apply(&clear).unwrap();
    let current = s.snapshot();
    assert_eq!(
        s.publish_prepared(prepared, || Ok(())).unwrap_err().code,
        ErrorCode::ConflictStaleBase
    );
    assert!(Arc::ptr_eq(&current, &s.snapshot()));
}

#[test]
fn admitted_paste_preview_retains_same_scope_and_styles_through_borrowed_render_input() {
    use paintcraft_core::types::{clipboard::Image, policy::BlendMode};
    use paintcraft_editor::clipboard::{PasteCanvas, PasteDraft};
    let mut doc = document();
    let layer = doc.roots[0];
    doc.layers.get_mut(&layer).unwrap().styles = StyleStack::new(vec![
        StyleInstance::new(Uuid::from_u128(1), true, descriptor([1, 2, 3])).unwrap(),
    ])
    .unwrap();
    let session = Session::new(doc);
    let original = session.snapshot();
    let image = Image::from_rgba(1, 1, vec![20, 60, 100, 128], &SharedBudget::new(4), &|| {
        Ok(())
    })
    .unwrap();
    let mut draft = PasteDraft::new(&session, image).unwrap();
    draft.place((0, 0), PasteCanvas::Keep).unwrap();
    let original_budget = SharedBudget::new(1 << 20);
    let scope = original_budget
        .reserve_capacity(1 << 20)
        .unwrap()
        .scope(AllocationClass::PaintMetadata);
    let adm = Admission::new(65536).with_paint_metadata_scope(scope.clone());
    assert_eq!(
        session
            .preview_paste_image_onto(
                &draft,
                layer,
                BlendMode::Replace,
                None,
                adm.clone(),
                &|| Ok(())
            )
            .unwrap_err()
            .code,
        ErrorCode::CapabilityUnsupported
    );
    let mut preview = session
        .preview_paste_image_onto_admitted(&draft, layer, BlendMode::Replace, None, adm, &|| Ok(()))
        .unwrap();
    assert!(scope.footprint().live > 0);
    assert_eq!(
        preview.document().layers[&layer].styles,
        original.layers[&layer].styles
    );
    assert_eq!(
        preview.document().layers[&layer].raster.get(0, 0),
        Rgba([20, 60, 100, 128])
    );
    assert!(preview.set_canvas((0, 1)).is_err());
    preview.set_canvas((1000, 486)).unwrap();
    assert!(Arc::ptr_eq(&original, &session.snapshot()));
    assert_eq!(session.undo_depth(), 0);
    drop(preview);
    assert_eq!(scope.footprint().live, 0);
    assert_eq!(original_budget.usage().reserved, 1 << 20);
}

#[test]
fn clear_canonical_background_removes_all_stored_style_intent_then_undo_restores_it() {
    for enabled in [false, true] {
        for opacity in [0, 255] {
            let mut doc = document();
            let layer = doc.roots[0];
            let entry = doc.layers.get_mut(&layer).unwrap();
            entry.name = "Background".into();
            entry.styles = StyleStack::new(vec![
                StyleInstance::new(
                    Uuid::from_u128(1),
                    enabled,
                    StyleDescriptor::ColorOverlayRgbV1(ColorOverlayRgbV1::new(
                        [220, 40, 0],
                        opacity,
                        EffectBlendMode::Normal,
                    )),
                )
                .unwrap(),
            ])
            .unwrap();
            assert_eq!(entry.raster.resident_tiles(), 0);
            let mut session = Session::new(doc);
            let before = session.snapshot();
            let clear = || {
                Value::object([
                    ("op", Value::str("canvas.clear")),
                    ("ref", Value::str("cleared")),
                    ("color", Value::str("#07090B00")),
                ])
            };
            let packet = batch(
                session.document(),
                session.ticket().publication_epoch,
                vec![clear()],
            );
            assert!(session.apply(&packet).unwrap().committed);
            assert!(!session.document().has_styles());
            assert_eq!(session.undo_depth(), 1);
            let cleared = session.snapshot();
            let id = cleared.roots[0];
            assert_ne!(id, layer);
            assert_eq!(
                cleared.layers[&id].raster.default_pixel(),
                Rgba([7, 9, 11, 0])
            );
            assert!(before.layers[&layer].styles.as_slice()[0].enabled() == enabled);
            assert_eq!(before.layers[&layer].styles.len(), 1);
            session.undo().unwrap();
            assert!(session.document().same_semantics(&before));
            session.redo().unwrap();
            assert!(session.document().same_semantics(&cleared));
            let ticket = session.ticket();
            let history = session.history_view();
            let source = session.snapshot();
            let packet = batch(session.document(), ticket.publication_epoch, vec![clear()]);
            assert!(!session.apply(&packet).unwrap().committed);
            assert!(Arc::ptr_eq(&source, &session.snapshot()));
            assert_eq!(session.ticket(), ticket);
            assert_eq!(session.history_view(), history);
        }
    }
}
