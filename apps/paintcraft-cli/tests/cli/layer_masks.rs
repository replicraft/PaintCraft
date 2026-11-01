//! Actual binary, staged selection capture, persistence and finite inspection.
use super::*;
use paintcraft_document::POLICY;
use serde_json::{Value as Json, json};
fn packet(source: &str, ops: Json) -> Json {
    let before: Json = serde_json::from_str(&run(&["inspect", source, "--json"]).stdout).unwrap();
    json!({"schema":"paintcraft.batch/layer-appearance-v1","atomic":true,"policy":POLICY,
        "base":{"state_id":before["ticket"]["state_id"],"publication_epoch":"0"},
        "required_filters":[],"required_layer_features":["layer-masks.v1"],"operations":ops})
}
fn apply(input: &str, opsfile: &str, output: &str, ops: Json) -> Run {
    fs::write(opsfile, serde_json::to_vec(&packet(input, ops)).unwrap()).unwrap();
    run(&[
        "apply",
        input,
        "--operations",
        opsfile,
        "-o",
        output,
        "--json",
    ])
}
fn inspect(path: &str) -> Json {
    let r = run(&["inspect", path, "--json"]);
    assert_eq!(r.code, 0, "{} {}", r.stdout, r.stderr);
    serde_json::from_str(&r.stdout).unwrap()
}
#[test]
fn cli_capture_from_real_atomic_selection_toggles_inverts_removes_and_inspects_pcnd7() {
    let dir = Scratch::new("own-layer-mask");
    let source = dir.path("source.pcnd");
    let ops = dir.path("ops.json");
    let raw = [240, 10, 20, 255, 30, 80, 200, 255];
    let original = paintcraft_document::import_image(2, 1, &raw, "layer-mask-cli-source").unwrap();
    fs::write(&source, paintcraft_formats::pcnd::write(&original).unwrap()).unwrap();
    let before = fs::read(&source).unwrap();
    let layer = original.roots[0].to_string();
    let output = dir.path("masked.pcnd");
    let first = apply(
        &source,
        &ops,
        &output,
        json!([
            {"op":"selection.set_rect","rect":{"x":0,"y":0,"width":1,"height":1}},
            {"op":"layer.mask.from_selection","layer_ref":{"id":layer},"enabled":true}
        ]),
    );
    assert_eq!(first.code, 0, "{} {}", first.stdout, first.stderr);
    let report: Json = serde_json::from_str(&first.stdout).unwrap();
    assert_eq!(report["schema"], "paintcraft.file-command/v5");
    assert_eq!(report["engine"]["required_filters"], json!([]));
    let bytes = fs::read(&output).unwrap();
    assert_eq!(bytes[4], 7);
    let doc = super::catalog::read_catalog(&bytes);
    let mask = doc.layers[&doc.roots[0]].mask.as_ref().unwrap();
    assert_eq!(
        [
            mask.coverage().coverage(0, 0),
            mask.coverage().coverage(1, 0)
        ],
        [255, 0]
    );
    let observed = inspect(&output);
    assert_eq!(observed["schema"], "paintcraft.inspect/v5");
    assert_eq!(observed["assessment"]["container_version"], 7);
    assert_eq!(observed["assessment"]["required_filters"], json!([]));
    assert_eq!(observed["document"]["layers"][0]["mask"]["enabled"], true);
    let budget = paintcraft_core::types::resources::SharedBudget::new(1 << 20);
    let admission = paintcraft_render::spatial::WorkspaceAdmission::new(
        budget.scope(paintcraft_core::types::resources::AllocationClass::SharedBudget),
        1 << 20,
        1_000_000,
    );
    let pixels = paintcraft_render::spatial::with_workspace(admission, || {
        paintcraft_render::render_flat(&doc)
    })
    .unwrap();
    assert_eq!(pixels, [240, 10, 20, 255, 0, 0, 0, 0]);
    assert_eq!(budget.usage().reserved, 0);
    // Prove the real caller-owned Legacy factory, not only the direct helper above.
    let flattened = dir.path("flattened.pcnd");
    let flatten = run(&["flatten", &output, "-o", &flattened, "--json"]);
    assert_eq!(flatten.code, 0, "{} {}", flatten.stdout, flatten.stderr);
    let projected_bytes = fs::read(&flattened).unwrap();
    assert_eq!(projected_bytes[4], 1);
    let projected = paintcraft_formats::pcnd::read(&projected_bytes).unwrap();
    assert_eq!(projected.roots.len(), 1);
    let projected_layer = &projected.layers[&projected.roots[0]];
    assert!(projected_layer.mask.is_none());
    assert!(projected_layer.filters.is_empty());
    assert_eq!(projected_layer.origin, (0, 0));
    assert_eq!(
        (
            projected_layer.raster.width(),
            projected_layer.raster.height()
        ),
        (2, 1)
    );
    assert_eq!(projected_layer.raster.get(0, 0).0, [240, 10, 20, 255]);
    assert_eq!(projected_layer.raster.get(1, 0).0, [0, 0, 0, 0]);
    assert_eq!(fs::read(&output).unwrap(), bytes);
    // Old filter-only consent still cannot authorize foreign own-mask baking.
    let foreign = dir.path("refused.png");
    let refused = run(&[
        "render",
        &output,
        "-o",
        &foreign,
        "--allow-loss",
        "filters-baked",
        "--json",
    ]);
    assert_eq!(refused.code, 3, "{} {}", refused.stdout, refused.stderr);
    let refusal: Json = serde_json::from_str(&refused.stdout).unwrap();
    assert_eq!(refusal["engine"]["code"], "capability.unsupported");
    assert_eq!(
        refusal["engine"]["message"],
        "Layer masks require a qualified appearance or persistence route"
    );
    assert!(!Path::new(&foreign).exists());
    assert_eq!(fs::read(&output).unwrap(), bytes);
    let disabled = dir.path("disabled.pcnd");
    assert_eq!(
        apply(
            &output,
            &ops,
            &disabled,
            json!([{"op":"layer.mask.set_enabled","layer_ref":{"id":layer},"enabled":false}])
        )
        .code,
        0
    );
    let inverted = dir.path("inverted.pcnd");
    assert_eq!(
        apply(
            &disabled,
            &ops,
            &inverted,
            json!([{"op":"layer.mask.invert","layer_ref":{"id":layer}}])
        )
        .code,
        0
    );
    let inverse = super::catalog::read_catalog(&fs::read(&inverted).unwrap());
    let mask = inverse.layers[&inverse.roots[0]].mask.as_ref().unwrap();
    assert!(!mask.enabled());
    assert_eq!(
        [
            mask.coverage().coverage(0, 0),
            mask.coverage().coverage(1, 0)
        ],
        [0, 255]
    );
    let removed = dir.path("removed.pcnd");
    assert_eq!(
        apply(
            &inverted,
            &ops,
            &removed,
            json!([{"op":"layer.mask.remove","layer_ref":{"id":layer}}])
        )
        .code,
        0
    );
    let downgraded = fs::read(&removed).unwrap();
    assert_eq!(downgraded[4], 1);
    let plain = paintcraft_formats::pcnd::read(&downgraded).unwrap();
    assert!(plain.layers[&plain.roots[0]].mask.is_none());
    assert_eq!(
        plain.layers[&plain.roots[0]].raster.get(0, 0).0,
        [240, 10, 20, 255]
    );
    assert_eq!(fs::read(&source).unwrap(), before);
}
#[test]
fn cli_no_persisted_selection_or_legacy_envelope_can_synthesize_layer_mask() {
    let dir = Scratch::new("own-mask-closed");
    let source = dir.path("source.pcnd");
    let ops = dir.path("ops.json");
    let output = dir.path("refused.pcnd");
    let doc = paintcraft_document::new_document(
        2,
        1,
        Some(paintcraft_core::types::Rgba::TRANSPARENT),
        0,
        POLICY,
    )
    .unwrap();
    fs::write(&source, paintcraft_formats::pcnd::write(&doc).unwrap()).unwrap();
    let before = fs::read(&source).unwrap();
    let layer = doc.roots[0].to_string();
    let value =
        json!([{"op":"layer.mask.from_selection","layer_ref":{"id":layer},"enabled":false}]);
    let guarded = packet(&source, value.clone());
    fs::write(&ops, serde_json::to_vec(&guarded).unwrap()).unwrap();
    let conflict = run(&[
        "apply",
        &source,
        "--operations",
        &ops,
        "-o",
        &output,
        "--expect-input-sha256",
        &"0".repeat(64),
        "--json",
    ]);
    assert_eq!(conflict.code, 4);
    assert!(!Path::new(&output).exists());
    assert_eq!(fs::read(&source).unwrap(), before);
    let failure = apply(&source, &ops, &output, value.clone());
    assert_eq!(failure.code, 2);
    assert!(!Path::new(&output).exists());
    for schema in [
        "paintcraft.batch/v1",
        "paintcraft.batch/v5",
        "paintcraft.batch/catalog-v1",
    ] {
        let mut p = packet(&source, value.clone());
        p["schema"] = json!(schema);
        fs::write(&ops, serde_json::to_vec(&p).unwrap()).unwrap();
        assert_eq!(
            run(&[
                "apply",
                &source,
                "--operations",
                &ops,
                "-o",
                &output,
                "--json"
            ])
            .code,
            2
        );
        assert!(!Path::new(&output).exists());
        assert_eq!(fs::read(&source).unwrap(), before);
    }
}
