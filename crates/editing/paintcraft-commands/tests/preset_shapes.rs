use paintcraft_commands::{Batch, Op, ShapeKind};
use paintcraft_core::types::{ErrorCode, shapes::ShapePreset};

fn batch(kind: &str, schema: &str) -> String {
    format!(
        r##"{{"schema":"paintcraft.batch/{schema}","atomic":true,"policy":"paintcraft.core/default-v1","base":{{"state_id":"11111111-1111-4111-8111-111111111111","publication_epoch":"0"}},"operations":[{{"op":"raster.shape","layer_ref":{{"id":"22222222-2222-4222-8222-222222222222"}},"kind":"{kind}","from":{{"x":1,"y":2}},"to":{{"x":30,"y":40}},"fill":"#12345680","stroke":{{"color":"#ABCDEF90","size":3}},"mode":"source-over"}}]}}"##
    )
}

#[test]
fn every_closed_preset_has_a_distinct_typed_v2_command_and_keeps_v1_unchanged() {
    let mut names = std::collections::BTreeSet::new();
    for shape in ShapePreset::ALL {
        assert!(names.insert(shape.command_name()));
        let parsed = Batch::from_json(batch(shape.command_name(), "v2").as_bytes()).unwrap();
        assert!(
            matches!(parsed.operations[0],Op::Shape{kind:ShapeKind::Preset(preset),..} if preset==shape)
        );
        assert_eq!(
            Batch::from_json(batch(shape.command_name(), "v1").as_bytes())
                .unwrap_err()
                .code,
            ErrorCode::InputSchema
        );
    }
    assert_eq!(
        Batch::from_json(batch("not-a-shape", "v2").as_bytes())
            .unwrap_err()
            .code,
        ErrorCode::InputSchema
    );
}
