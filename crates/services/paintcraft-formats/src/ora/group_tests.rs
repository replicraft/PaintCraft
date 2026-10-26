use super::*;
const SOURCE: &[u8] = include_bytes!("../../tests/ora-group-fixtures/neutral.ora");
#[test]
fn group_projection_buffers_are_admitted_before_pixel_decode() {
    // 22 source pixels *4 + two3x2 comparison buffers *4 =136 bytes.
    let limits = Limits {
        pixels: 64,
        decoded_bytes: 135,
        ..Limits::default()
    };
    assert_eq!(
        decode_neutral_groups(SOURCE, &limits, &|| Ok(()))
            .err()
            .unwrap()
            .code,
        ErrorCode::ResourceLimit
    );
    let copy = decode_neutral_groups(
        SOURCE,
        &Limits {
            decoded_bytes: 136,
            ..limits
        },
        &|| Ok(()),
    )
    .unwrap();
    assert_eq!(copy.merged.unwrap().len(), 24);
    assert_eq!(copy.decoded.losses, [GROUP_STRUCTURE_LOSS]);
    // A thumbnail can exceed canvas dimensions; its live-buffer price is separate.
    let large = include_bytes!("../../tests/ora-group-fixtures/large-thumbnail.ora");
    assert_eq!(
        decode_neutral_groups(
            large,
            &Limits {
                decoded_bytes: 136,
                ..limits
            },
            &|| Ok(())
        )
        .err()
        .unwrap()
        .code,
        ErrorCode::ResourceLimit
    );
    assert!(
        decode_neutral_groups(
            large,
            &Limits {
                decoded_bytes: 280,
                ..limits
            },
            &|| Ok(())
        )
        .is_ok()
    );
}
