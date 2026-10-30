use super::*;
fn batch(version: u8, operation: &str) -> Result<Batch> {
    Batch::from_json(format!(r#"{{"schema":"paintcraft.batch/v{version}","atomic":true,"policy":"paintcraft.core/default-v1","base":{{"state_id":"00000000-0000-4000-8000-000000000001","publication_epoch":"0"}},"operations":[{operation}]}}"#).as_bytes())
}
const ADD: &str = r#"{"op":"filter.add","layer_ref":{"ref":"ink"},"ref":"invert","descriptor":{"kind":"invert-rgb-v1"}}"#;
#[test]
fn six_operations_have_strict_v2_refs_indices_parameters_and_false_enabled() {
    let all=[ADD.to_owned(),
        r#"{"op":"filter.update","layer_ref":{"ref":"ink"},"filter_ref":{"ref":"invert"},"descriptor":{"kind":"brightness-contrast-v1","brightness":-100,"contrast":99}}"#.into(),
        r#"{"op":"filter.set_enabled","layer_ref":{"ref":"ink"},"filter_ref":{"ref":"invert"},"enabled":false}"#.into(),
        r#"{"op":"filter.reorder","layer_ref":{"ref":"ink"},"filter_ref":{"id":"00000000-0000-4000-8000-000000000002"},"to_index":0}"#.into(),
        r#"{"op":"filter.remove","layer_ref":{"ref":"ink"},"filter_ref":{"ref":"invert"}}"#.into(),
        r#"{"op":"filter.duplicate","layer_ref":{"ref":"ink"},"filter_ref":{"ref":"invert"},"ref":"copy","to_index":256}"#.into()];
    for operation in all {
        assert!(matches!(
            batch(2, &operation).unwrap().operations[0],
            Op::Filter(_)
        ));
        assert_eq!(
            batch(1, &operation).unwrap_err().code,
            ErrorCode::InputSchema
        );
    }
    assert!(matches!(
        &batch(2, ADD).unwrap().operations[0],
        Op::Filter(FilterOp::Add {
            enabled: true,
            to_index: None,
            ..
        })
    ));
    let disabled = ADD.replace("\"descriptor\":", "\"enabled\":false,\"descriptor\":");
    assert!(matches!(
        &batch(2, &disabled).unwrap().operations[0],
        Op::Filter(FilterOp::Add { enabled: false, .. })
    ));
}
#[test]
fn malformed_descriptors_fields_numeric_types_and_union_refs_refuse() {
    let bright = ADD.replace(
        r#"{"kind":"invert-rgb-v1"}"#,
        r#"{"kind":"brightness-contrast-v1","brightness":0,"contrast":0}"#,
    );
    for invalid in [
        ADD.replace("invert-rgb-v1","invert-rgb-v2"),
        ADD.replace(r#""kind":"invert-rgb-v1""#,r#""kind":"invert-rgb-v1","brightness":0"#),
        ADD.replace(r#""ref":"invert""#,r#""ref":"BadAlias"#),
        ADD.replace(r#""ref":"invert""#,r#""ref":"invert","extra":0"#),
        ADD.replace(r#""descriptor":"#,r#""enabled":null,"descriptor":"#),
        ADD.replace(r#""descriptor":"#,r#""to_index":-1,"descriptor":"#),
        ADD.replace(r#""descriptor":"#,r#""to_index":257,"descriptor":"#),
        bright.replace(r#""contrast":0"#,r#""contrast":100"#),
        bright.replace(r#""brightness":0"#,r#""brightness":101"#),
        bright.replace(r#""brightness":0"#,r#""brightness":0.5"#),
        bright.replace(r#""brightness":0,"#, ""),
        r#"{"op":"filter.remove","layer_ref":{"ref":"ink"},"filter_ref":{"ref":"invert","id":"00000000-0000-4000-8000-000000000001"}}"#.into(),
    ]{assert_eq!(batch(2,&invalid).unwrap_err().code,ErrorCode::InputSchema,"{invalid}");}
}
