//! Actual stdio discovery/apply/history/save; literal independent complete image.
use super::*;
use paintcraft_document::Document;
use std::{fs, path::Path};
const EXPECTED: [u8; 36] = [
    0, 0, 0, 0, 17, 34, 51, 0, 0, 0, 0, 0, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255,
    255, 0, 0, 0, 0, 255, 255, 255, 255, 0, 0, 0, 0,
];
fn save(c: &mut Client, path: &Path) -> Document {
    c.ok(
        "save_project",
        json!({"path":path.to_str().unwrap(),"overwrite":true}),
    );
    paintcraft_formats::pcnd::read(&fs::read(path).unwrap()).unwrap()
}
fn raw(d: &Document) -> Vec<u8> {
    let l = &d.layers[&d.roots[0]];
    let mut rgba = Vec::new();
    for y in 0..l.raster.height() {
        for x in 0..l.raster.width() {
            rgba.extend(l.raster.get(i64::from(x), i64::from(y)).0)
        }
    }
    rgba
}
fn error(result: &Value) -> Value {
    assert_eq!(result["isError"], true, "{result}");
    serde_json::from_str(result["content"][0]["text"].as_str().unwrap()).unwrap()
}
#[test]
fn genuine_rotation_schema_atomic_pixels_save_undo_redo_and_invalid_norm() {
    let folder =
        std::env::temp_dir().join(format!("paintcraft-mcp-rotation-{}", uuid::Uuid::new_v4()));
    fs::create_dir(&folder).unwrap();
    let mut c = Client::new();
    let old = c.ok("get_batch_schema", json!({}));
    let old_expected: Value =
        serde_json::from_str(include_str!("../../schemas/batch-v5.schema.json")).unwrap();
    assert_eq!(old, old_expected);
    let schema = c.ok("get_rotation_schema", json!({}));
    let expected: Value =
        serde_json::from_str(include_str!("../../schemas/rotation-v1/batch.schema.json")).unwrap();
    assert_eq!(schema, expected);
    assert_eq!(
        schema["properties"]["schema"]["const"],
        "paintcraft.batch/rotation-v1"
    );
    c.ok(
        "create_canvas",
        json!({"width":2,"height":2,"background":"#FFFFFFFF"}),
    );
    let initial = c.ok("inspect_document", json!({}));
    let layer = initial["layers"][0]["id"].clone();
    let applied=c.ok("apply_operations",json!({"operations":[{"op":"raster.fill_rect","layer_ref":{"id":layer},"rect":{"x":0,"y":0,"width":1,"height":1},"color":"#11223300","mode":"replace"},{"op":"canvas.rotate","coefficients":[644245094,858993459]}]}));
    assert_eq!(applied["committed"], true);
    let rotated = c.ok("inspect_document", json!({}));
    let project = folder.join("rotated.pcnd");
    let d = save(&mut c, &project);
    assert_eq!(d.canvas, (3, 3));
    assert_eq!(raw(&d), EXPECTED);
    let saved_bytes = fs::read(&project).unwrap();
    let stable = c.ok("inspect_document", json!({}));
    // Both operations parse together before any source publication: the prefix
    // would change a real pixel if invalid rotation were applied independently.
    let invalid=c.call("apply_operations",json!({"operations":[{"op":"raster.fill_rect","layer_ref":{"id":layer},"rect":{"x":0,"y":0,"width":1,"height":1},"color":"#ABCDEF12","mode":"replace"},{"op":"canvas.rotate","coefficients":[0,0]}]}));
    assert_eq!(error(&invalid)["code"], "input.schema");
    assert_eq!(c.ok("inspect_document", json!({})), stable);
    assert_eq!(fs::read(&project).unwrap(), saved_bytes);
    // Rotation does not broaden the preexisting explicit MCP blend-edit gate.
    let denied=c.call("apply_operations",json!({"operations":[{"op":"canvas.rotate","coefficients":[0,1073741824]},{"op":"layer.set_blend_mode","layer_ref":{"id":layer},"blend_mode":"multiply"}]}));
    assert_eq!(error(&denied)["code"], "capability.unsupported");
    assert_eq!(c.ok("inspect_document", json!({})), stable);
    c.ok("history", json!({}));
    let restored = save(&mut c, &folder.join("undo.pcnd"));
    assert_eq!(restored.canvas, (2, 2));
    assert_eq!(raw(&restored), [255; 16]);
    c.ok("history", json!({"redo":true}));
    let redone = save(&mut c, &folder.join("redo.pcnd"));
    assert_eq!(redone.canvas, (3, 3));
    assert_eq!(raw(&redone), EXPECTED);
    assert_eq!(redone.state_id, d.state_id);
    assert_eq!(
        c.ok("inspect_document", json!({}))["state_id"],
        rotated["state_id"]
    );
    drop(c);
    fs::remove_dir_all(folder).unwrap();
}
