//! Negative protocol injection only; actual successful decode is separately required.
use super::*;
use std::cell::Cell;
pub(super) fn assert_bound_preview_refusals(
    encoded: &wire::native::PreparedJxrEncode,
    owner: wire::Owner,
) {
    let raw = fs::read(
        Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../paintcraft-cli/tests/jxr-export-fixtures/4x3.rgba"),
    )
    .unwrap();
    let original = encoded.sha256();
    for case in 0..5 {
        let consumed = Cell::new(false);
        let result = encoded.consume_decoded_preview(
            Duration::from_secs(10),
            &|| Ok(()),
            &|| Ok(owner),
            |launch, _| {
                let mut header = launch.request.header;
                if case == 0 {
                    header.nonce = Uuid::new_v4();
                }
                if case == 1 {
                    header.owner.epoch = header.owner.epoch.wrapping_add(1);
                }
                let mut rgba = raw.clone();
                if case == 2 {
                    header.sha256[0] ^= 1;
                }
                if case == 4 {
                    rgba[0] ^= 1;
                }
                let (width, height) = if case == 3 { (3, 4) } else { (4, 3) };
                wire::success(
                    header,
                    wire::Success {
                        width,
                        height,
                        pixels: wire::Pixels::SrgbRgba8(&rgba),
                    },
                    0,
                )
            },
            |_, _, _, _| {
                consumed.set(true);
                Ok(())
            },
        );
        let error = result.unwrap_err();
        assert_eq!(
            error.code,
            if case < 3 {
                ErrorCode::ConflictStaleBase
            } else {
                ErrorCode::CodecFailed
            },
            "case{case}: {error}"
        );
        assert!(!consumed.get());
        assert_eq!(encoded.sha256(), original);
    }
}
