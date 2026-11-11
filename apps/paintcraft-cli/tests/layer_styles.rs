//! Actual file commands, explicit consent, canonical profiles and independently decoded full PNGs.
#[path = "support/layer_styles.rs"]
mod support;
use serde_json::{Value, json};
use std::fs;
use support::{EXPECTED_COMPOUND, Folder, add, base, compound, packet, png, run, success};

fn apply(
    folder: &Folder,
    doc: &paintcraft_document::Document,
    ops: Value,
    features: Value,
) -> (std::path::PathBuf, Value) {
    let source = folder.path("source.pcnd");
    let out = folder.path("styled.pcnd");
    let batch = folder.path("ops.json");
    fs::write(
        &source,
        paintcraft_formats::pcnd::write_compact(doc).unwrap(),
    )
    .unwrap();
    fs::write(
        &batch,
        serde_json::to_vec(&packet(doc, ops, features)).unwrap(),
    )
    .unwrap();
    let before = fs::read(&source).unwrap();
    let result = success(run(&[
        "apply",
        source.to_str().unwrap(),
        "--operations",
        batch.to_str().unwrap(),
        "-o",
        out.to_str().unwrap(),
        "--json",
    ]));
    assert_eq!(fs::read(source).unwrap(), before);
    (out, result)
}
#[test]
fn actual_apply_inspect_schema_and_styled_png_consent_are_separate() {
    let f = Folder::new();
    let doc = base();
    let id = doc.roots[0];
    let (out, result) = apply(
        &f,
        &doc,
        json!([{"op":"raster.fill_rect","layer_ref":{"id":id.to_string()},"rect":{"x":1,"y":0,"width":1,"height":1},"color":"#6496C8FF","mode":"replace"},add(id,[255,0,0],255)]),
        json!(["layer-styles.v1"]),
    );
    assert_eq!(result["schema"], "paintcraft.file-command/layer-styles-v1");
    assert_eq!(
        result["engine"]["schema"],
        "paintcraft.result/layer-styles-v1"
    );
    assert!(result["engine"]["style_aliases"]["overlay"]["style_id"].is_string());
    let bytes = fs::read(&out).unwrap();
    assert_eq!(bytes[4], 9);
    let inspected = success(run(&["inspect", out.to_str().unwrap(), "--json"]));
    assert_eq!(inspected["schema"], "paintcraft.inspect/layer-styles-v1");
    assert_eq!(
        inspected["document"]["layers"][0]["styles"][0]["descriptor"]["color"],
        json!([255, 0, 0])
    );
    assert_eq!(inspected["assessment"]["container_version"], 9);
    let png_path = f.path("copy.png");
    for extra in [
        vec![],
        vec!["--allow-loss", "styles-baked"],
        vec!["--allow-loss", "filters-baked"],
        vec!["--appearance-copy", "bake"],
    ] {
        let mut args = vec![
            "render",
            out.to_str().unwrap(),
            "-o",
            png_path.to_str().unwrap(),
            "--json",
        ];
        args.extend(extra);
        let failed = run(&args);
        assert!(
            !failed.status.success(),
            "{}",
            String::from_utf8_lossy(&failed.stdout)
        );
        assert!(!png_path.exists());
    }
    let rendered = success(run(&[
        "render",
        out.to_str().unwrap(),
        "-o",
        png_path.to_str().unwrap(),
        "--appearance-copy",
        "bake",
        "--allow-loss",
        "styles-baked",
        "--json",
    ]));
    assert_eq!(
        rendered["engine"]["schema"],
        "paintcraft.export-disclosure/layer-styles-v1"
    );
    assert_eq!(rendered["engine"]["counts"]["styles"], 1);
    assert_eq!(rendered["engine"]["styles_baked"], true);
    assert_eq!(
        png(&fs::read(png_path).unwrap()),
        [255, 0, 0, 255, 255, 0, 0, 255]
    );
    assert_eq!(fs::read(&out).unwrap(), bytes);
    let foreign = f.path("copy.bmp");
    assert!(
        !run(&[
            "render",
            out.to_str().unwrap(),
            "-o",
            foreign.to_str().unwrap(),
            "--appearance-copy",
            "bake",
            "--allow-loss",
            "styles-baked",
            "--json"
        ])
        .status
        .success()
    );
    assert!(!foreign.exists());
}
#[test]
fn compound_filters_style_own_mask_opacity_and_multiply_export_full_pixels() {
    let f = Folder::new();
    let doc = compound();
    let id = *doc.roots.last().unwrap();
    let mut batch = packet(
        &doc,
        json!([add(id, [220, 40, 0], 128)]),
        json!(["layer-styles.v1"]),
    );
    // Existing filter intention is source metadata; required_filters declares operation descriptors only.
    batch["required_filters"] = json!([]);
    let source = f.path("source.pcnd");
    let out = f.path("styled.pcnd");
    let ops = f.path("ops.json");
    fs::write(
        &source,
        paintcraft_formats::pcnd::write_compact(&doc).unwrap(),
    )
    .unwrap();
    fs::write(&ops, serde_json::to_vec(&batch).unwrap()).unwrap();
    success(run(&[
        "apply",
        source.to_str().unwrap(),
        "--operations",
        ops.to_str().unwrap(),
        "-o",
        out.to_str().unwrap(),
        "--json",
    ]));
    let image = f.path("compound.png");
    let before = fs::read(&out).unwrap();
    let receipt = success(run(&[
        "render",
        out.to_str().unwrap(),
        "-o",
        image.to_str().unwrap(),
        "--appearance-copy",
        "bake",
        "--allow-loss",
        "styles-baked,filters-baked,layer-appearance-baked,flatten",
        "--json",
    ]));
    let disclosure = &receipt["engine"];
    assert_eq!(
        disclosure["counts"],
        json!({"styles":1,"filters":1,"filter_masks":1,"layer_masks":1,"non_normal_layers":1})
    );
    assert_eq!(disclosure["filter_masks_baked"], true);
    assert_eq!(disclosure["layer_masks_baked"], true);
    assert_eq!(disclosure["layer_blends_baked"], true);
    assert_eq!(png(&fs::read(image).unwrap()), EXPECTED_COMPOUND);
    assert_eq!(fs::read(out).unwrap(), before);
}
#[test]
fn closed_profiles_refuse_downgrade_unknown_fields_and_leave_destination_unchanged() {
    let f = Folder::new();
    let doc = base();
    let source = f.path("base.pcnd");
    let dest = f.path("sentinel.pcnd");
    let ops = f.path("ops.json");
    fs::write(
        &source,
        paintcraft_formats::pcnd::write_compact(&doc).unwrap(),
    )
    .unwrap();
    let good = packet(
        &doc,
        json!([add(doc.roots[0], [17, 93, 201], 0)]),
        json!(["layer-styles.v1"]),
    );
    for kind in 0..4 {
        let mut invalid = good.clone();
        match kind {
            0 => invalid["schema"] = json!("paintcraft.batch/v2"),
            1 => invalid["required_layer_features"] = json!([]),
            2 => invalid["operations"][0]["descriptor"]["unexpected"] = json!(true),
            _ => invalid["operations"][0]["descriptor"]["opacity_u8"] = json!(256),
        };
        fs::write(&ops, serde_json::to_vec(&invalid).unwrap()).unwrap();
        fs::write(&dest, b"sentinel").unwrap();
        assert!(
            !run(&[
                "apply",
                source.to_str().unwrap(),
                "--operations",
                ops.to_str().unwrap(),
                "-o",
                dest.to_str().unwrap(),
                "--overwrite",
                "--json"
            ])
            .status
            .success()
        );
        assert_eq!(fs::read(&dest).unwrap(), b"sentinel");
    }
    for (alias, schema) in [
        ("batch-layer-styles", "paintcraft.batch/layer-styles-v1"),
        ("result-layer-styles", "paintcraft.result/layer-styles-v1"),
        ("inspect-layer-styles", "paintcraft.inspect/layer-styles-v1"),
    ] {
        let discovered = success(run(&["schema", alias]));
        assert_eq!(discovered["properties"]["schema"]["const"], schema);
    }
}
#[path = "support/mcp_client.rs"]
mod mcp_client;
#[path = "layer_styles/stdio.rs"]
mod stdio;

#[test]
fn capabilities_report_implemented_style_routes_without_claiming_qualification() {
    let value = success(run(&["capabilities", "--json"]));
    let rows = value["capabilities"]
        .as_array()
        .expect("capability inventory");
    for id in [
        "layer.styles.color-overlay.edit",
        "pcnd.layer-styles.read",
        "pcnd.layer-styles.write",
        "png.styled-appearance.copy",
    ] {
        let row = rows.iter().find(|row| row["capability"] == id).unwrap();
        assert_eq!(row["implementation"], "implemented");
        assert_eq!(row["release_qualified"], false);
        assert_eq!(row["evidence"], "unverified");
    }
}
