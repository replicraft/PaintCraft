use super::*;
const OPAQUE: &[u8] = include_bytes!("../../../tests/avif-fixtures/opaque.avif");
const ALPHA: &[u8] = include_bytes!("../../../tests/avif-fixtures/alpha.avif");
const NAMED: &[u8] = include_bytes!("../../../tests/avif-fixtures/named-alpha.avif");
#[test]
fn exact_source_geometry_color_alpha_names_and_digest_are_sealed() {
    for (source, alpha) in [(OPAQUE, false), (ALPHA, true)] {
        let assessment = assess_import(source, 256, &|| Ok(())).unwrap();
        assert_eq!(assessment.size(), (16, 16));
        assert_eq!(assessment.source_len(), source.len() as u64);
        assert_eq!(
            assessment.sha256(),
            <[u8; 32]>::from(Sha256::digest(source))
        );
        assert_eq!(assessment.primary_id(), 1);
        assert_eq!(assessment.primary_name(), "");
        assert_eq!(assessment.alpha(), alpha);
        assert_eq!(assessment.alpha_id(), alpha.then_some(2));
        assert_eq!(assessment.alpha_name(), alpha.then_some(""));
        assert_eq!(assessment.color_policy(), COLOR_POLICY);
        assert!(assessment.losses().is_empty());
        assessment.require_losses(&[]).unwrap();
        let seq = assessment.primary_sequence();
        assert_eq!((seq.profile, seq.level, seq.bit_depth), (1, 31, 8));
        assert_eq!(
            (seq.primaries, seq.transfer, seq.matrix, seq.full_range),
            (1, 13, 0, true)
        );
        assert_eq!(assessment.alpha_sequence().is_some(), alpha);
        let receipt = String::from_utf8(paintcraft_core::types::json::canonical(
            &assessment.to_value(),
        ))
        .unwrap();
        assert!(receipt.contains("\"preserving_save\":false"));
        assert!(receipt.contains("\"sequence_level_index\":31"));
        assert!(receipt.contains(&hex(&assessment.sha256())));
    }
    let assessment = assess_import(NAMED, 256, &|| Ok(())).unwrap();
    assert_eq!(assessment.primary_name(), "Color image");
    assert_eq!(assessment.alpha_name(), Some("Alpha mask"));
    assert_eq!(assessment.losses(), KNOWN_IMPORT_LOSSES);
    assert_ne!(
        assessment.sha256(),
        assess_import(ALPHA, 256, &|| Ok(())).unwrap().sha256()
    );
}
#[test]
fn item_names_require_precise_consent_without_inventing_identity_color_loss() {
    let assessment = assess_import(NAMED, 256, &|| Ok(())).unwrap();
    assert_eq!(assessment.losses(), ["avif-item-names-not-retained"]);
    assert_eq!(
        assessment.require_losses(&[]).unwrap_err().code,
        ErrorCode::CapabilityUnsupported
    );
    assessment
        .require_losses(&["avif-item-names-not-retained".into()])
        .unwrap();
    assert_eq!(
        assessment
            .require_losses(&["heif-item-names-not-retained".into()])
            .unwrap_err()
            .code,
        ErrorCode::InputSchema
    );
    let opaque = include_bytes!("../../../tests/avif-fixtures/named-opaque.avif");
    assert_eq!(
        assess_import(opaque, 256, &|| Ok(())).unwrap().losses(),
        KNOWN_IMPORT_LOSSES
    );
    let original = admit_srgb8(ALPHA, 256, &|| Ok(())).unwrap();
    let named = admit_srgb8(NAMED, 256, &|| Ok(())).unwrap();
    assert_eq!(
        original.primary.parts.concat(),
        named.primary.parts.concat()
    );
    assert_eq!(
        original.alpha.unwrap().parts.concat(),
        named.alpha.unwrap().parts.concat()
    );
}
#[test]
fn malformed_sources_cancel_and_budget_do_not_create_assessments() {
    assert_eq!(
        assess_import(ALPHA, 255, &|| Ok(())).unwrap_err().code,
        ErrorCode::ResourceLimit
    );
    assert_eq!(
        assess_import(ALPHA, 256, &|| Err(Error::new(
            ErrorCode::OperationCancelled,
            "cancel"
        )))
        .unwrap_err()
        .code,
        ErrorCode::OperationCancelled
    );
    let mut invalid = ALPHA.to_vec();
    let color = invalid
        .windows(4)
        .position(|value| value == b"colr")
        .unwrap()
        + 4;
    invalid[color..color + 4].copy_from_slice(b"prof");
    assert_eq!(
        assess_import(&invalid, 256, &|| Ok(())).unwrap_err().code,
        ErrorCode::CapabilityUnsupported
    );
    assert!(assess_import(&ALPHA[..ALPHA.len() - 1], 256, &|| Ok(())).is_err());
}
