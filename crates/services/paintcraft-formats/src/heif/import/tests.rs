use super::*;
const IDENTITY: &[u8] = include_bytes!("../../../tests/heif-color-fixtures/identity444-alpha.heic");
const YUV: &[u8] = include_bytes!("../../../tests/heif-color-fixtures/yuv420-limited-alpha.heic");
#[test]
fn exact_source_color_and_alpha_declarations_are_bound_to_sha_and_actual_losses() {
    for (bytes, chroma, matrix, range) in [(IDENTITY, 3, 0, true), (YUV, 1, 1, false)] {
        let assessment = assess_import(bytes, PIXELS_MAX, &|| Ok(())).unwrap();
        assert_eq!(assessment.size(), (32, 24));
        assert_eq!(assessment.chroma(), chroma);
        assert_eq!(
            assessment.color(),
            NativeEncoding {
                primaries: 1,
                transfer: 13,
                matrix,
                full_range: range
            }
        );
        assert!(assessment.alpha());
        assert_eq!(assessment.sha256(), <[u8; 32]>::from(Sha256::digest(bytes)));
        assert_eq!(
            assessment
                .to_value()
                .get("source_sha256")
                .and_then(Value::as_str),
            Some(hex(&Sha256::digest(bytes)).as_str())
        );
        assert!(assessment.require_losses(&["unknown".into()]).is_err());
        assert_eq!(assessment.require_losses(&[]).is_ok(), matrix == 0);
        assessment
            .require_losses(&[KNOWN_IMPORT_LOSSES[0].into()])
            .unwrap();
        assert_eq!(assessment.losses().is_empty(), matrix == 0);
    }
}
#[test]
fn unqualified_metadata_color_extents_and_cancellation_cannot_reach_adoption() {
    let mut unknown = IDENTITY.to_vec();
    let at = unknown
        .windows(4)
        .position(|bytes| bytes == b"pixi")
        .unwrap();
    unknown[at..at + 4].copy_from_slice(b"junk");
    assert!(assess_import(&unknown, PIXELS_MAX, &|| Ok(())).is_err());
    let mut color = IDENTITY.to_vec();
    let at = color.windows(4).position(|bytes| bytes == b"nclx").unwrap();
    color[at + 7] = 2;
    assert!(assess_import(&color, PIXELS_MAX, &|| Ok(())).is_err());
    assert!(assess_import(&IDENTITY[..IDENTITY.len() - 1], PIXELS_MAX, &|| Ok(())).is_err());
    assert_eq!(
        assess_import(IDENTITY, 767, &|| Ok(())).unwrap_err().code,
        ErrorCode::ResourceLimit
    );
    assert_eq!(
        assess_import(IDENTITY, PIXELS_MAX, &|| Err(Error::new(
            ErrorCode::OperationCancelled,
            "cancel"
        )))
        .unwrap_err()
        .code,
        ErrorCode::OperationCancelled
    );
    assert!(
        assess_import(
            include_bytes!("../../../tests/heif-fixtures/primary.heic"),
            PIXELS_MAX,
            &|| Ok(())
        )
        .is_err()
    );
}
