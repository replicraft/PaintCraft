use super::*;
use paintcraft_core::types::resources::SharedBudget;
use paintcraft_document::source_metadata::*;
const ICC: &[u8] = include_bytes!("../../../../../assets/profiles/sRGB2014.icc");
fn attached() -> Document {
    let descriptor = SourceDescriptor {
        format: SourceFormat::Png,
        source_model: SourceColorModel::Rgb,
        source_sha256: [7; 32],
        source_bytes: 4096,
        selection: None,
        source_dimensions: (3, 2),
        output_dimensions: (2, 3),
        orientation: Some(6),
        orientation_policy: OrientationPolicy::ExifMaterializedV1,
        resolution: Some(Resolution {
            horizontal: Rational {
                numerator: -3780,
                denominator: 1,
            },
            vertical: Rational {
                numerator: 30000,
                denominator: 100,
            },
            unit: ResolutionUnit::Metre,
            raw_unit: Some(1),
        }),
    };
    let metadata = SourceMetadata::capture(
        descriptor,
        Some(ICC),
        BYTES_MAX,
        &SharedBudget::new(BYTES_MAX),
        &|| Ok(()),
    )
    .unwrap();
    paintcraft_document::new_document(
        2,
        3,
        Some(Rgba([17, 33, 55, 127])),
        0,
        paintcraft_document::POLICY,
    )
    .unwrap()
    .with_source_metadata_checked(Some(metadata), &|| Ok(()))
    .unwrap()
}

#[test]
fn independent_python_pcnd2_fixtures_retain_all_source_facts_and_exact_raw_bytes() {
    for name in [
        "rgb-profile.pcnd",
        "resolution-only.pcnd",
        "orientation-only.pcnd",
        "legacy-v1.pcnd",
    ] {
        let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("tests/pcnd-metadata-fixtures")
            .join(name);
        let bytes = std::fs::read(path).unwrap();
        let document = read(&bytes).unwrap();
        let mut output = Vec::new();
        write_to(&document, &mut output).unwrap();
        assert_eq!(output, bytes, "independent bytes: {name}");
        if name == "legacy-v1.pcnd" {
            assert!(document.source_metadata.is_none());
            continue;
        }
        assert_eq!(
            document.document_id,
            Uuid::parse_str("33333333-3333-4333-8333-333333333333").unwrap()
        );
        assert_eq!(
            document.state_id,
            Uuid::parse_str("11111111-1111-4111-8111-111111111111").unwrap()
        );
        let metadata = document.source_metadata.as_ref().unwrap();
        let d = metadata.descriptor();
        assert_eq!(d.format, SourceFormat::Tiff);
        assert_eq!(d.source_model, SourceColorModel::Rgb);
        assert_eq!(d.selection, Some(7));
        assert_eq!(
            d.source_sha256,
            <[u8; 32]>::from(Sha256::digest(
                b"Independent PCND2 original-source placeholder"
            ))
        );
        assert_eq!(
            d.source_bytes,
            b"Independent PCND2 original-source placeholder".len() as u64
        );
        assert_eq!(d.source_dimensions, (3, 2));
        assert_eq!(d.output_dimensions, (2, 3));
        assert_eq!(d.orientation, Some(6));
        assert_eq!(d.orientation_policy, OrientationPolicy::ExifMaterializedV1);
        assert_eq!(
            metadata.profile(),
            (name == "rgb-profile.pcnd").then_some(ICC)
        );
        assert_eq!(metadata.working_color(), WorkingColor::UnmanagedRgba8);
        if name == "orientation-only.pcnd" {
            assert!(d.resolution.is_none());
        } else {
            let r = d.resolution.unwrap();
            assert_eq!(
                r.horizontal,
                Rational {
                    numerator: 30000,
                    denominator: 100
                }
            );
            assert_eq!(
                r.vertical,
                Rational {
                    numerator: -3780,
                    denominator: 1
                }
            );
            assert_eq!(r.unit, ResolutionUnit::Inch);
            assert_eq!(r.raw_unit, Some(2));
            assert!(r.physical().is_none());
        }
    }
}

#[test]
fn real_canvas_edits_history_and_reopen_preserve_original_conversion_geometry() {
    use paintcraft_commands::{Anchor, Batch, Op};
    use paintcraft_core::types::{Rect, orthogonal::OrthogonalTransform};
    use paintcraft_editor::session::Session;
    let initial = attached();
    let original = initial.source_metadata.clone();
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
        assert_ne!(session.document().state_id, states.last().unwrap().state_id);
        assert_eq!(session.document().source_metadata, original);
        assert_eq!(
            session
                .document()
                .source_metadata
                .as_ref()
                .unwrap()
                .descriptor()
                .output_dimensions,
            (2, 3)
        );
        let restored = read(&write_compact(session.document()).unwrap()).unwrap();
        assert!(restored.same_semantics(session.document()));
        states.push(session.snapshot());
    }
    assert_eq!(session.document().canvas, (3, 4));
    session.compact_history(1, 1 << 20, || Ok(())).unwrap();
    for state in states[..states.len() - 1].iter().rev() {
        session.undo().unwrap().unwrap();
        assert!(session.document().same_semantics(state));
        assert_eq!(session.document().source_metadata, original);
    }
    for state in &states[1..] {
        session.redo().unwrap().unwrap();
        assert!(session.document().same_semantics(state));
        let restored = read(&write(session.document()).unwrap()).unwrap();
        assert!(restored.same_semantics(state));
    }
}

#[test]
fn aggregate_prelude_limit_precedes_profile_copy_and_later_failure_releases_attachment() {
    let bytes = include_bytes!("../../tests/pcnd-metadata-fixtures/rgb-profile.pcnd");
    let at = bytes.windows(8).position(|p| p == b"PCSMETA1").unwrap();
    let mut header: [u8; WIRE_HEADER_BYTES] = bytes[at..at + WIRE_HEADER_BYTES].try_into().unwrap();
    let mut profile = ICC.to_vec();
    profile.resize(BYTES_MAX as usize - WIRE_HEADER_BYTES, 0);
    let size = profile.len() as u32;
    profile[..4].copy_from_slice(&size.to_be_bytes());
    header[112..120].copy_from_slice(&(profile.len() as u64).to_be_bytes());
    header[120..152].copy_from_slice(&Sha256::digest(&profile));
    header[152..184].fill(0);
    let mut hash = Sha256::new();
    hash.update(header);
    hash.update(&profile);
    header[152..184].copy_from_slice(&hash.finalize());
    let mut hostile = bytes[..at].to_vec();
    hostile.extend(header);
    hostile.extend(profile);
    hostile.push(0);
    hostile.extend([0; 32]);
    digest(&mut hostile);
    let budget = SharedBudget::new(BYTES_MAX + 4096);
    let error = read_with_metadata_admission_checked(
        &hostile,
        u64::MAX,
        STAGED_PIXELS_MAX,
        LAYER_COUNT_MAX,
        MetadataAdmission {
            bytes: BYTES_MAX,
            budget: &budget,
        },
        &|| Ok(()),
    )
    .unwrap_err();
    assert_eq!(error.code, ErrorCode::ResourceLimit);
    assert_eq!(
        budget.usage().peak,
        0,
        "prelude bytes must count BEFORE metadata copy/admission"
    );
    let mut late = bytes.to_vec();
    let end = late.len();
    assert_eq!(late[end - 33], 0);
    late[end - 33] = 1;
    digest(&mut late);
    assert!(
        read_with_metadata_admission_checked(
            &late,
            u64::MAX,
            STAGED_PIXELS_MAX,
            LAYER_COUNT_MAX,
            MetadataAdmission {
                bytes: BYTES_MAX,
                budget: &budget
            },
            &|| Ok(())
        )
        .is_err()
    );
    assert!(
        budget.usage().peak > 0,
        "valid attachment was admitted before the truncated layer record"
    );
    assert_eq!(budget.usage().reserved, 0);
}

#[test]
fn header_rejects_unknown_required_features_and_missing_attachment_on_v2() {
    let mut header = probe(&write(&attached()).unwrap()).unwrap();
    header.require_supported().unwrap();
    header.required_features.push("future.unknown".into());
    assert_eq!(
        header.require_supported().unwrap_err().code,
        ErrorCode::CapabilityUnsupported
    );
    let plain =
        paintcraft_document::new_document(2, 3, None, 0, paintcraft_document::POLICY).unwrap();
    let mut bytes = write(&plain).unwrap();
    bytes[4] = 2;
    digest(&mut bytes);
    assert!(read(&bytes).is_err());
    let mut legacy = probe(&write(&plain).unwrap()).unwrap();
    legacy
        .required_features
        .push(SOURCE_METADATA_FEATURE.into());
    assert!(legacy.require_supported().is_err());
}

#[test]
fn preserving_foreign_writers_refuse_attached_metadata_before_expensive_preflight() {
    let doc = attached();
    let fail = || panic!("Metadata policy must refuse before foreign writer work");
    let paint = crate::paint::preflight(
        &doc,
        crate::paint::Options {
            canvas_fill: [0; 4],
            layer_names: crate::paint::LayerNames::Omit,
        },
        &crate::paint::Limits::default(),
        &fail,
    )
    .err()
    .unwrap();
    let pdn = crate::pdn::preflight(
        &doc,
        crate::pdn::Options {
            background: crate::pdn::BackgroundRole::Ordinary,
            geometry: crate::pdn::Geometry::Materialize,
        },
        &crate::pdn::Limits::default(),
        &fail,
    )
    .err()
    .unwrap();
    let psd = crate::psd::preflight(
        &doc,
        crate::psd::Version::Psd,
        &crate::psd::WriteLimits::default(),
        &fail,
    )
    .err()
    .unwrap();
    for error in [paint, pdn, psd] {
        assert_eq!(error.code, ErrorCode::CapabilityUnsupported);
        assert!(error.message.contains("Source metadata"));
    }
}
#[test]
fn pcnd2_round_trip_retains_exact_profile_source_facts_and_identifiers() {
    let document = attached();
    for bytes in [write(&document).unwrap(), write_compact(&document).unwrap()] {
        assert_eq!(bytes[4], 2);
        assert_eq!(probe(&bytes).unwrap().container_version, 2);
        let restored = read(&bytes).unwrap();
        assert!(document.same_semantics(&restored));
        assert_eq!(document.state_id, restored.state_id);
        assert_eq!(
            restored.source_metadata.as_ref().unwrap().profile(),
            Some(ICC)
        );
        assert!(
            restored
                .source_metadata
                .as_ref()
                .unwrap()
                .descriptor()
                .resolution
                .unwrap()
                .physical()
                .is_none()
        );
    }
    let plain =
        paintcraft_document::new_document(2, 3, None, 0, paintcraft_document::POLICY).unwrap();
    let bytes = write(&plain).unwrap();
    assert_eq!(bytes[4], 1);
    assert!(read(&bytes).unwrap().source_metadata.is_none());
}
fn digest(bytes: &mut [u8]) {
    let n = bytes.len() - 32;
    let hash = Sha256::digest(&bytes[..n]);
    bytes[n..].copy_from_slice(&hash);
}
#[test]
fn pcnd2_host_metadata_limits_and_corrupt_extents_refuse_without_retained_charge() {
    let bytes = write(&attached()).unwrap();
    let budget = SharedBudget::new(BYTES_MAX);
    assert_eq!(
        read_with_metadata_admission_checked(
            &bytes,
            u64::MAX,
            STAGED_PIXELS_MAX,
            LAYER_COUNT_MAX,
            MetadataAdmission {
                bytes: 1,
                budget: &budget
            },
            &|| Ok(())
        )
        .unwrap_err()
        .code,
        ErrorCode::ResourceLimit
    );
    assert_eq!(budget.usage().reserved, 0);
    let at = bytes.windows(8).position(|p| p == b"PCSMETA1").unwrap();
    for field in [at + 112, at + 152, at + 188] {
        let mut bad = bytes.clone();
        bad[field] ^= 255;
        digest(&mut bad);
        assert!(
            read_with_metadata_admission_checked(
                &bad,
                u64::MAX,
                STAGED_PIXELS_MAX,
                LAYER_COUNT_MAX,
                MetadataAdmission {
                    bytes: BYTES_MAX,
                    budget: &budget
                },
                &|| Ok(())
            )
            .is_err()
        );
        assert_eq!(budget.usage().reserved, 0);
    }
    let restored = read_with_metadata_admission_checked(
        &bytes,
        u64::MAX,
        STAGED_PIXELS_MAX,
        LAYER_COUNT_MAX,
        MetadataAdmission {
            bytes: BYTES_MAX,
            budget: &budget,
        },
        &|| Ok(()),
    )
    .unwrap();
    assert!(budget.usage().reserved > 0);
    drop(restored);
    assert_eq!(budget.usage().reserved, 0);
}
