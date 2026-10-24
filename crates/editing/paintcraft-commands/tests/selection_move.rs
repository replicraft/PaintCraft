use paintcraft_commands::{Batch, LayerRef, Op};
use paintcraft_core::types::{ErrorCode, Rgba};

fn parse(version: u8, operation: &str) -> paintcraft_core::types::Result<Batch> {
    Batch::from_json(format!(r#"{{"schema":"paintcraft.batch/v{version}","atomic":true,"policy":"paintcraft.core/default-v1","base":{{"state_id":"00000000-0000-4000-8000-000000000001","publication_epoch":"0"}},"operations":[{operation}]}}"#).as_bytes())
}

#[test]
fn selected_movement_has_strict_v2_offsets_colors_and_optional_rgb_key() {
    let plain = r##"{"op":"raster.move_selection","layer_ref":{"ref":"base"},"offset":{"x":-7,"y":3},"clear":"#11223300"}"##;
    assert_eq!(
        parse(2, plain).unwrap().operations[0],
        Op::MoveSelection {
            layer: LayerRef::Alias("base".into()),
            offset: (-7, 3),
            clear: Rgba([17, 34, 51, 0]),
            transparent_key: None,
        }
    );
    let keyed = plain.replace("\"clear\":", "\"transparent_key\":\"#AABBCC42\",\"clear\":");
    assert!(matches!(
        parse(2, &keyed).unwrap().operations[0],
        Op::MoveSelection {
            transparent_key: Some(Rgba([170, 187, 204, 66])),
            ..
        }
    ));
    assert_eq!(parse(1, plain).unwrap_err().code, ErrorCode::InputSchema);
    for invalid in [
        plain.replace("\"x\":-7", "\"x\":0.5"),
        plain.replace("\"x\":-7", "\"x\":140737488355328"),
        plain.replace("\"x\":-7", "\"x\":-140737488355328"),
        plain.replace("\"x\":-7,", ""),
        plain.replace("\"y\":3", "\"y\":3,\"z\":0"),
        plain.replace("\"clear\":", "\"other\":0,\"clear\":"),
        plain.replace("\"clear\":\"#11223300\"", "\"clear\":null"),
        plain.replace("\"clear\":", "\"transparent_key\":null,\"clear\":"),
        plain.replace("\"clear\":", "\"transparent_key\":\"white\",\"clear\":"),
        plain.replace("\"offset\":", "\"offsets\":"),
    ] {
        assert_eq!(
            parse(2, &invalid).unwrap_err().code,
            ErrorCode::InputSchema,
            "{invalid}"
        );
    }
}
