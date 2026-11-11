//! Actual stdio tools exercise the shared typed batch and actual compound PNG exporter.
use super::{
    apply,
    mcp_client::Client,
    support::{self, Folder},
};
use serde_json::json;
use std::{fs, path::Path};
fn client() -> Client {
    Client::at(Path::new(env!("CARGO_BIN_EXE_paintcraft-cli")))
}
#[test]
fn six_style_operations_noops_history_save_reopen_and_last_remove_use_explicit_profile() {
    let folder = Folder::new();
    let source = folder.path("source.pcnd");
    let saved = folder.path("styled.pcnd");
    let removed = folder.path("removed.pcnd");
    let doc = support::base();
    let layer = doc.roots[0];
    fs::write(
        &source,
        paintcraft_formats::pcnd::write_compact(&doc).unwrap(),
    )
    .unwrap();
    let original = fs::read(&source).unwrap();
    let mut c = client();
    let schemas = c.ok("get_layer_style_schema", json!({"kind":"batch"}));
    assert_eq!(
        schemas["properties"]["schema"]["const"],
        support::STYLE_SCHEMA
    );
    c.ok("open_project", json!({"path":source}));
    let before = c.ok("inspect_document", json!({}));
    let added = c.ok(
        "apply_operations",
        json!({"operations":[support::add(layer,[255,0,0],255)]}),
    );
    assert_eq!(added["schema"], "paintcraft.result/layer-styles-v1");
    let id = added["style_aliases"]["overlay"]["style_id"].clone();
    let inspected = c.ok("inspect_document", json!({}));
    assert_eq!(
        inspected["schema"],
        "paintcraft.mcp-inspect/layer-styles-v1"
    );
    assert_eq!(inspected["document_features"], json!(["layer-styles.v1"]));
    assert_eq!(inspected["required_filters"], json!([]));
    let noop=c.ok("apply_operations",json!({"operations":[{"op":"layer.style.update","layer_ref":{"id":layer.to_string()},"style_ref":{"id":id},"descriptor":support::descriptor([255,0,0],255)}]}));
    assert_eq!(noop["committed"], false);
    assert_eq!(noop["schema"], "paintcraft.result/layer-styles-v1");
    assert_eq!(c.ok("inspect_document", json!({})), inspected);
    let edited=c.ok("apply_operations",json!({"operations":[
        {"op":"layer.style.duplicate","layer_ref":{"id":layer.to_string()},"style_ref":{"id":id},"ref":"duplicate"},
        {"op":"layer.style.move","layer_ref":{"id":layer.to_string()},"style_ref":{"ref":"duplicate"},"to_index":0},
        {"op":"layer.style.set_enabled","layer_ref":{"id":layer.to_string()},"style_ref":{"ref":"duplicate"},"enabled":false},
        {"op":"layer.style.update","layer_ref":{"id":layer.to_string()},"style_ref":{"ref":"duplicate"},"descriptor":support::descriptor([0,255,0],0)}]}));
    assert_eq!(edited["committed"], true);
    let duplicate = edited["style_aliases"]["duplicate"]["style_id"].clone();
    assert_ne!(duplicate, id);
    let ordered = c.ok("inspect_document", json!({}));
    assert_eq!(ordered["layers"][0]["styles"][0]["id"], duplicate);
    assert_eq!(ordered["layers"][0]["styles"][0]["enabled"], false);
    let disabled = folder.path("disabled-zero.pcnd");
    c.ok("save_project", json!({"path":disabled}));
    assert_eq!(fs::read(disabled).unwrap()[4], 9);

    c.ok("history", json!({}));
    assert_eq!(
        c.ok("inspect_document", json!({}))["layers"],
        inspected["layers"]
    );
    c.ok("history", json!({"redo":true}));
    c.ok("apply_operations",json!({"operations":[{"op":"layer.style.remove","layer_ref":{"id":layer.to_string()},"style_ref":{"id":duplicate}}]}));
    c.ok("save_project", json!({"path":saved}));
    assert_eq!(fs::read(&saved).unwrap()[4], 9);
    c.ok("open_project", json!({"path":saved}));
    let reopened = c.ok("inspect_document", json!({}));
    assert_eq!(reopened["layers"], inspected["layers"]);
    let last=c.ok("apply_operations",json!({"operations":[{"op":"layer.style.remove","layer_ref":{"id":layer.to_string()},"style_ref":{"id":id}}]}));
    assert_eq!(last["schema"], "paintcraft.result/layer-styles-v1");
    assert_eq!(last["document_features"], json!([]));
    assert_eq!(last["style_aliases"], json!({}));
    c.ok("save_project", json!({"path":removed}));
    assert_eq!(fs::read(removed).unwrap()[4], 1);
    assert_eq!(fs::read(source).unwrap(), original);
    assert_ne!(before["state_id"], reopened["state_id"]);
}
#[test]
fn styled_compound_stdio_copy_requires_its_own_grant_and_preserves_source_and_dirty_state() {
    let folder = Folder::new();
    let doc = support::compound();
    let (source, _) = apply(
        &folder,
        &doc,
        json!([support::add(*doc.roots.last().unwrap(), [220, 40, 0], 128)]),
        json!(["layer-styles.v1"]),
    );
    let before = fs::read(&source).unwrap();
    let image = folder.path("appearance.png");
    let mut c = client();
    c.ok("open_project", json!({"path":source}));
    let inspected = c.ok("inspect_document", json!({}));
    for args in [
        json!({"path":image,"allow_losses":["flatten","filters-baked","styles-baked","layer-appearance-baked"]}),
        json!({"path":image,"appearance_copy":"bake","allow_losses":["flatten","filters-baked","styles-baked"]}),
        json!({"path":image,"appearance_copy":"refuse"}),
    ] {
        assert_eq!(c.call("export_image", args)["isError"], true);
        assert!(!image.exists());
        assert_eq!(c.ok("inspect_document", json!({})), inspected);
    }
    let output=c.ok("export_image",json!({"path":image,"appearance_copy":"bake","allow_losses":["flatten","filters-baked","styles-baked","layer-appearance-baked"]}));
    assert_eq!(output["schema"], "paintcraft.file-command/layer-styles-v1");
    assert_eq!(
        output["engine"]["schema"],
        "paintcraft.export-disclosure/layer-styles-v1"
    );
    assert_eq!(
        output["engine"]["counts"],
        json!({"styles":1,"filters":1,"filter_masks":1,"layer_masks":1,"non_normal_layers":1})
    );
    assert_eq!(
        support::png(&fs::read(image).unwrap()),
        support::EXPECTED_COMPOUND
    );
    assert_eq!(fs::read(source).unwrap(), before);
    assert_eq!(c.ok("inspect_document", json!({})), inspected);
}
