use super::*;

#[test]
fn full_attachment_identity_is_deterministic_and_canvas_history_and_pcnd_preserve_it() {
    use paintcraft_commands::{Anchor, Batch, Op};
    use paintcraft_core::types::{Rect, orthogonal::OrthogonalTransform};
    use paintcraft_editor::session::Session;
    let bytes = own("rgba32-v124-profile-hidden.bmp");
    let budget = SharedBudget::new(1 << 20);
    let converted = prepared(&bytes, &budget, &|| Ok(())).unwrap();
    let initial = document(&converted).unwrap();
    let repeat =
        document(&prepared(&bytes, &SharedBudget::new(1 << 20), &|| Ok(())).unwrap()).unwrap();
    assert_eq!(
        (initial.document_id, initial.state_id, initial.roots.clone()),
        (repeat.document_id, repeat.state_id, repeat.roots.clone())
    );
    let old = paintcraft_document::import_image(
        3,
        2,
        &converted.image.rgba,
        &hex(&Sha256::digest(&bytes)),
    )
    .unwrap();
    assert_ne!(initial.document_id, old.document_id);
    let mut changed = bytes.clone();
    changed[38..42].copy_from_slice(&(-3780i32).to_le_bytes());
    let other =
        document(&prepared(&changed, &SharedBudget::new(1 << 20), &|| Ok(())).unwrap()).unwrap();
    assert_ne!(other.document_id, initial.document_id);
    let expected = own("rgba32-v124-profile-hidden.bmp.rgba");
    let layer = initial.layers.get(&initial.roots[0]).unwrap();
    let actual: Vec<u8> = (0..2)
        .flat_map(|y| (0..3).flat_map(move |x| layer.raster.get(x, y).0))
        .collect();
    assert_eq!(
        actual, expected,
        "authoritative layer retains hidden RGB, not a composited render"
    );
    let attachment = initial.source_metadata.clone();
    let mut session = Session::new(initial);
    let mut states = vec![session.snapshot()];
    for (index, operation) in [
        Op::CanvasResize {
            width: 5,
            height: 4,
            anchor: Anchor::TopLeft,
        },
        Op::CanvasCrop {
            rect: Rect::new(0, 0, 4, 3).unwrap(),
        },
        Op::CanvasTransform {
            transform: OrthogonalTransform::RotateRight,
        },
    ]
    .into_iter()
    .enumerate()
    {
        let batch = Batch {
            version: 2,
            policy: paintcraft_document::POLICY.into(),
            base_state: session.document().state_id,
            base_epoch: session.ticket().publication_epoch,
            seed: None,
            digest: format!("{:064x}", index + 1),
            operations: vec![operation],
        };
        assert!(session.apply(&batch).unwrap().committed);
        assert_eq!(session.document().source_metadata, attachment);
        assert_eq!(
            attachment.as_ref().unwrap().descriptor().source_dimensions,
            (3, 2)
        );
        let reopened =
            crate::pcnd::read(&crate::pcnd::write_compact(session.document()).unwrap()).unwrap();
        assert!(reopened.same_semantics(session.document()));
        states.push(session.snapshot());
    }
    for prior in states[..states.len() - 1].iter().rev() {
        session.undo().unwrap().unwrap();
        assert!(prior.same_semantics(session.document()));
    }
    for next in states[1..].iter() {
        session.redo().unwrap().unwrap();
        assert!(next.same_semantics(session.document()));
    }
}

#[test]
fn initial_document_checks_the_actual_pcnd_boundary_without_arbitrary_headroom() {
    let small = prepared(
        &own("rgb24-v124-profile-positive.bmp"),
        &SharedBudget::new(1 << 20),
        &|| Ok(()),
    )
    .unwrap();
    let mut baseline = document(&small).unwrap();
    baseline.source_metadata = None;
    // Raw PCND1 has one24-byte clipped tile and a32-byte trailer. Everything
    // else counts against metadata; PCND2 adds only its192-byte profile header.
    let overhead = crate::pcnd::write(&baseline).unwrap().len() - 24 - 32;
    let exact = BYTES_MAX as usize - overhead - 192;
    for extra in [0, 1] {
        let bytes = with_profile_size(exact + extra);
        let budget = SharedBudget::new(BYTES_MAX + 4096);
        let capture = prepared(&bytes, &budget, &|| Ok(())).unwrap();
        let result = document(&capture);
        assert_eq!(result.is_ok(), extra == 0);
        if let Ok(document) = result {
            let actual = crate::pcnd::write(&document).unwrap();
            assert_eq!(actual.len(), BYTES_MAX as usize + 24 + 32);
            assert!(
                crate::pcnd::read(&actual)
                    .unwrap()
                    .same_semantics(&document)
            );
        } else {
            assert_eq!(result.unwrap_err().code, ErrorCode::ResourceLimit);
        }
        drop(capture);
        assert_eq!(budget.usage().reserved, 0);
    }
}

#[test]
fn default_flat_exports_refuse_bmp_attachment_and_explicit_copy_keeps_private_authoring_state() {
    use crate::export::{
        self,
        metadata_policy::{Intent, Policy, Target},
    };
    use crate::registry::{Family, Operation};
    let prepared = prepared(
        &own("rgb24-v124-profile-positive.bmp"),
        &SharedBudget::new(1 << 20),
        &|| Ok(()),
    )
    .unwrap();
    let document = document(&prepared).unwrap();
    let before = crate::pcnd::write_compact(&document).unwrap();
    let allowed = export::KNOWN_LOSSES
        .iter()
        .map(|s| s.to_string())
        .collect::<Vec<_>>();
    let error = export::export_flat_options_with(
        &document,
        export::FlatOptions::new(ImageFormat::Png, 90),
        &allowed,
        |_| panic!("default refusal must precede rendering"),
    )
    .unwrap_err();
    assert_eq!(error.code, ErrorCode::CapabilityUnsupported);
    let mut options = export::FlatOptions::new(ImageFormat::Png, 90);
    options.source_metadata = Policy::capture(
        &document,
        Target::new(Family::Png, Operation::ExportFlat),
        Intent::Omit,
    )
    .unwrap();
    let output = export::export_flat_options(&document, options, &[]).unwrap();
    assert_eq!(output.losses, [export::metadata_policy::OMITTED_REASON]);
    assert_eq!(
        super::super::super::decode(&output.bytes).unwrap().rgba,
        prepared.image.rgba
    );
    assert_eq!(before, crate::pcnd::write_compact(&document).unwrap());
}
