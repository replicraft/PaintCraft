use paintcraft_commands::{Batch, Op};
use paintcraft_core::types::ErrorCode;
fn parse(version: u8, op: &str) -> paintcraft_core::types::Result<Batch> {
    Batch::from_json(format!(r#"{{"schema":"paintcraft.batch/v{version}","atomic":true,"policy":"paintcraft.core/default-v1","base":{{"state_id":"00000000-0000-4000-8000-000000000001","publication_epoch":"0"}},"operations":[{op}]}}"#).as_bytes())
}
#[test]
fn polygon_and_inversion_require_v2_and_strict_bounded_geometry() {
    let polygon =
        r#"{"op":"selection.set_polygon","points":[{"x":-1,"y":2},{"x":4,"y":2},{"x":2,"y":6}]}"#;
    assert!(matches!(
        parse(2, polygon).unwrap().operations[0],
        Op::SelectionSetPolygon { .. }
    ));
    assert!(matches!(
        parse(2, r#"{"op":"selection.invert"}"#).unwrap().operations[0],
        Op::SelectionInvert
    ));
    for op in [polygon, r#"{"op":"selection.invert"}"#] {
        assert_eq!(parse(1, op).unwrap_err().code, ErrorCode::InputSchema);
    }
    for op in [
        r#"{"op":"selection.invert","domain":{}}"#,
        r#"{"op":"selection.set_polygon","points":[]}"#,
        r#"{"op":"selection.set_polygon","points":[{"x":0,"y":0},{"x":1,"y":0}]}"#,
        r#"{"op":"selection.set_polygon","points":[{"x":0.5,"y":0},{"x":1,"y":0},{"x":1,"y":1}]}"#,
        r#"{"op":"selection.set_polygon","points":[{"x":0,"y":0,"z":1},{"x":1,"y":0},{"x":1,"y":1}]}"#,
    ] {
        assert_eq!(parse(2, op).unwrap_err().code, ErrorCode::InputSchema);
    }
    let points = vec![r#"{"x":0,"y":0}"#; 65_537].join(",");
    assert!(
        parse(
            2,
            &format!(r#"{{"op":"selection.set_polygon","points":[{points}]}}"#)
        )
        .is_err()
    );
}
