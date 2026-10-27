use paintcraft_commands::{Batch, Op};
use paintcraft_core::types::ErrorCode;
fn parse(extra: &str) -> paintcraft_core::types::Result<Batch> {
    Batch::from_json(format!(r##"{{"schema":"paintcraft.batch/v2","atomic":true,"policy":"paintcraft.core/default-v1","base":{{"state_id":"00000000-0000-4000-8000-000000000001","publication_epoch":"0"}},"operations":[{{"op":"raster.mask_spans","layer_ref":{{"ref":"background"}},"color":"#00000000","spans":[[0,0,1,127]]{extra}}}]}}"##).as_bytes())
}
#[test]
fn mask_mode_is_optional_source_over_and_strict_when_present() {
    assert!(matches!(
        parse("").unwrap().operations[0],
        Op::MaskSpans {
            mode: paintcraft_commands::MaskMode::SourceOver,
            ..
        }
    ));
    assert!(matches!(
        parse(r#", "mode":"source-over""#).unwrap().operations[0],
        Op::MaskSpans {
            mode: paintcraft_commands::MaskMode::SourceOver,
            ..
        }
    ));
    assert!(matches!(
        parse(r#", "mode":"replace""#).unwrap().operations[0],
        Op::MaskSpans {
            mode: paintcraft_commands::MaskMode::Replace,
            ..
        }
    ));
    assert!(matches!(
        parse(r#", "mode":"destination-out""#).unwrap().operations[0],
        Op::MaskSpans {
            mode: paintcraft_commands::MaskMode::DestinationOut,
            ..
        }
    ));
    for invalid in [
        r#", "mode":null"#,
        r#", "mode":"erase""#,
        r#", "modes":"replace""#,
    ] {
        assert_eq!(parse(invalid).unwrap_err().code, ErrorCode::InputSchema);
    }
}
