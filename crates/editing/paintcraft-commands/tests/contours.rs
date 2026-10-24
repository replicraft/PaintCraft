use paintcraft_commands::{Batch, Op};
use paintcraft_core::types::{
    ErrorCode,
    json::{self, Value},
};

fn command() -> Value {
    json::parse(br##"{"schema":"paintcraft.batch/v2","atomic":true,"policy":"paintcraft.core/default-v1","base":{"state_id":"11111111-1111-4111-8111-111111111111","publication_epoch":"0"},"operations":[{"op":"raster.contour","layer_ref":{"ref":"background"},"points":[{"x":1,"y":1},{"x":10,"y":1},{"x":10,"y":10}],"closed":true,"stroke":{"color":"#12345680","size":3},"fill":"#34567890","mode":"source-over"}]}"##).unwrap()
}
fn operation(value: &mut Value) -> &mut std::collections::BTreeMap<String, Value> {
    let Value::Object(top) = value else { panic!() };
    let Value::Array(ops) = top.get_mut("operations").unwrap() else {
        panic!()
    };
    let Value::Object(op) = &mut ops[0] else {
        panic!()
    };
    op
}
#[test]
fn contour_parser_checks_closed_open_and_unknown_fields_without_relaxing_v1() {
    let good = command();
    assert!(matches!(
        Batch::from_value(&good).unwrap().operations[0],
        Op::Contour { closed: true, .. }
    ));
    let mut open = good.clone();
    let op = operation(&mut open);
    op.insert("closed".into(), Value::Bool(false));
    op.remove("fill");
    assert!(Batch::from_value(&open).is_ok());
    for kind in 0..7 {
        let mut value = good.clone();
        let op = operation(&mut value);
        match kind {
            0 => {
                op.insert("closed".into(), Value::Bool(false));
            }
            1 => {
                op.insert("closed".into(), Value::Int(1));
            }
            2 => {
                op.insert("points".into(), Value::Array(vec![]));
            }
            3 => {
                op.remove("stroke");
                op.remove("fill");
            }
            4 => {
                op.insert("extra".into(), Value::Bool(true));
            }
            5 => {
                let Value::Array(points) = op.get_mut("points").unwrap() else {
                    panic!()
                };
                points.truncate(2);
            }
            _ => {
                let Value::Object(top) = &mut value else {
                    panic!()
                };
                top.insert("schema".into(), Value::str("paintcraft.batch/v1"));
            }
        }
        assert_eq!(
            Batch::from_value(&value).unwrap_err().code,
            ErrorCode::InputSchema,
            "case {kind}"
        );
    }
}
