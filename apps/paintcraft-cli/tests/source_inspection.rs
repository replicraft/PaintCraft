//! Read-only compiled CLI and real stdio MCP source facts against owned oracles.
use serde_json::{Value, json};
use sha2::{Digest, Sha256};
use std::{
    fs,
    path::{Path, PathBuf},
    process::Command,
};
#[path = "support/mcp_client.rs"]
mod mcp_client;
fn fixtures() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../crates/services/paintcraft-formats/tests")
}
fn hash(bytes: &[u8]) -> String {
    paintcraft_document::hex(&Sha256::digest(bytes))
}
fn cli(path: &Path, extra: &[&str]) -> std::process::Output {
    Command::new(env!("CARGO_BIN_EXE_paintcraft-cli"))
        .arg("inspect-source")
        .arg(path)
        .arg("--json")
        .args(extra)
        .output()
        .unwrap()
}
fn client() -> mcp_client::Client {
    mcp_client::Client::at(Path::new(env!("CARGO_BIN_EXE_paintcraft-cli")))
}
fn value(output: std::process::Output) -> Value {
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    serde_json::from_slice(&output.stdout).unwrap()
}
fn equal_losses(value: &Value, expected: &[&str]) {
    let mut a: Vec<_> = value
        .as_array()
        .unwrap()
        .iter()
        .map(|v| v.as_str().unwrap())
        .collect();
    a.sort_unstable();
    let mut b = expected.to_vec();
    b.sort_unstable();
    assert_eq!(a, b);
}
fn assert_common(report: &Value, bytes: &[u8], format: &str) {
    assert_eq!(report["schema"], "paintcraft.source-inspection/v1");
    assert_eq!(report["format"], format);
    assert_eq!(report["source"]["sha256"], hash(bytes));
    assert_eq!(report["source"]["bytes"], bytes.len());
    assert_eq!(report["inspection_only"], true);
    for key in [
        "pixel_decode_performed",
        "authorizes_adoption",
        "metadata_retained_in_document",
        "color_management_performed",
    ] {
        assert_eq!(report[key], false);
    }
    assert!(report.get("ticket").is_none());
    assert!(report.get("document").is_none());
    assert_eq!(
        report["proposed_conversion"]["requires_pixel_validation"],
        true
    );
}
#[test]
fn actual_cli_and_stdio_report_original_facts_and_exact_header_known_conversion_policies() {
    let mut mcp = client();
    let tools = mcp.request("tools/list", json!({}));
    let tool = tools["tools"]
        .as_array()
        .unwrap()
        .iter()
        .find(|t| t["name"] == "inspect_source")
        .unwrap();
    assert_eq!(tool["annotations"]["readOnlyHint"], true);
    assert_eq!(tool["annotations"]["destructiveHint"], false);
    let cases: [(&str, &str, &[&str]); 11] = [
        (
            "raster-metadata-fixtures/indexed4-trns.png",
            "png",
            &["palette-indices-expanded"],
        ),
        (
            "raster-metadata-fixtures/rgba16-high-byte.png",
            "png",
            &["depth-16-reduced-to-8", "png-metadata-ignored"],
        ),
        (
            "raster-metadata-fixtures/rgba8-icc-precedence.png",
            "png",
            &[
                "icc-source-retained-unapplied",
                "png-color-metadata-ignored",
                "png-metadata-ignored",
            ],
        ),
        (
            "raster-metadata-fixtures/rgba8-exif-orientation.png",
            "png",
            &["exif-metadata-ignored"],
        ),
        (
            "bmp-rle-fixtures/rle8-padding.bmp",
            "bmp",
            &["palette-indices-expanded", "resolution-metadata-ignored"],
        ),
        (
            "bmp-rle-fixtures/rle4-size-zero.bmp",
            "bmp",
            &["palette-indices-expanded", "bmp-metadata-ignored"],
        ),
        (
            "bmp-rle-fixtures/rle4-icc-before.bmp",
            "bmp",
            &[
                "palette-indices-expanded",
                "icc-profile-ignored",
                "bmp-color-metadata-ignored",
            ],
        ),
        ("bmp16-fixtures/masks565-v40-bottom.bmp", "bmp", &[]),
        ("bmp16-fixtures/rgb555-v40-top.bmp", "bmp", &[]),
        (
            "jpeg-fixtures/unmarked-cmyk-orientation-6-II.jpg",
            "jpeg",
            &[
                "exif-metadata-ignored",
                "jpeg-cmyk-to-rgb",
                "jpeg-color-model-assumed",
            ],
        ),
        (
            "jpeg-fixtures/adobe-ycck-color.jpg",
            "jpeg",
            &["jpeg-cmyk-to-rgb"],
        ),
    ];
    for (name, format, losses) in cases {
        let path = fixtures().join(name);
        let bytes = fs::read(&path).unwrap();
        let cli = value(cli(&path, &[]));
        let peer = mcp.ok("inspect_source", json!({"path":path}));
        assert_eq!(cli, peer);
        assert_common(&cli, &bytes, format);
        equal_losses(&cli["proposed_conversion"]["losses"], losses);
        if format == "png" {
            assert_eq!(
                cli["proposed_conversion"]["identity_profile"],
                "png-source-profile-resolution-rgba8-v1"
            );
            let policy = &cli["proposed_conversion"]["source_attachment_policy"];
            assert_eq!(policy["requires_attachment_admission"], true);
            assert_eq!(policy["requires_final_import_validation"], true);
            assert_eq!(policy["color_management_applied"], false);
            assert_eq!(policy["active_print_settings"], false);
            assert_eq!(
                policy["icc_profile_bytes"],
                name.ends_with("rgba8-icc-precedence.png")
            );
        } else {
            assert!(cli["proposed_conversion"]["source_attachment_policy"].is_null());
        }

        let facts = &cli["facts"];
        if name.contains("indexed4-trns") {
            assert_eq!(facts["component_depth"], 4);
            assert_eq!(facts["components"], 1);
            assert_eq!(facts["palette"]["rgb"][0], json!([13, 71, 201]));
            assert_eq!(facts["palette"]["alpha"][0], 0);
            assert_eq!(facts["palette"]["indices_retained"], false);
        }
        if name.contains("rgba16-high-byte") {
            assert_eq!(facts["component_depth"], 16);
            assert_eq!(
                cli["proposed_conversion"]["depth_policy"],
                "strip16-high-byte"
            );
        }
        if name.contains("icc-") && format == "png" {
            let manifest: Value = serde_json::from_slice(
                &fs::read(fixtures().join("raster-metadata-fixtures/oracle.json")).unwrap(),
            )
            .unwrap();
            assert_eq!(facts["icc"]["payload"]["sha256"], manifest["icc_sha256"]);
            assert_eq!(facts["color_declaration_precedence"], "icc");
        }
        if name.contains("exif-orientation.png") {
            assert!(facts["exif_orientation"].is_null());
            assert!(facts["exif_historical_payload"]["bytes"].as_u64().unwrap() > 0);
            assert_eq!(cli["proposed_conversion"]["width"], facts["width"]);
            assert_eq!(cli["proposed_conversion"]["height"], facts["height"]);
            assert_eq!(
                cli["proposed_conversion"]["orientation_policy"],
                "as-stored; eXIf is historical metadata"
            );
        }
        if name.contains("padding.bmp") {
            assert!(facts["row_stride"].is_null());
            assert_eq!(facts["resolution_raw"], json!([-3780, 0]));
            assert!(facts["resolution"].is_null());
            assert_eq!(facts["palette_reserved"][0], 137);
            assert_eq!(facts["palette_reserved_is_alpha"], false);
            assert_eq!(
                facts["pixel_data_bytes"].as_u64().unwrap(),
                facts["rle_stream_bytes"].as_u64().unwrap() + 2
            );
        }
        if name.contains("size-zero.bmp") {
            assert_eq!(facts["declared_image_bytes"], 0);
            assert_eq!(facts["pixel_data_bytes"], facts["rle_stream_bytes"]);
        }
        if name.contains("masks565") {
            assert_eq!(facts["bits_per_pixel"], 16);
            assert!(facts["component_depth"].is_null());
            assert_eq!(facts["component_depths"], json!([5, 6, 5]));
            assert_eq!(facts["masks"], json!([63488, 2016, 31, 0]));
        }
        if name.contains("rgb555") {
            assert_eq!(facts["bits_per_pixel"], 16);
            assert_eq!(facts["component_depth"], 5);
            assert_eq!(facts["component_depths"], json!([5, 5, 5]));
            assert_eq!(facts["masks"], json!([0, 0, 0, 0]));
            assert_eq!(facts["top_down"], true);
        }
        if name.contains("unmarked-cmyk") {
            assert_eq!(facts["width"], 24);
            assert_eq!(facts["height"], 16);
            assert_eq!(facts["orientation"], 6);
            assert_eq!(facts["model"], "cmyk");
            assert_eq!(facts["model_origin"], "component-count-convention");
            assert_eq!(cli["proposed_conversion"]["width"], 16);
            assert_eq!(cli["proposed_conversion"]["height"], 24);
            assert_eq!(
                cli["proposed_conversion"]["orientation_policy"],
                "jpeg-orientation-materialized-v1"
            );
        }
        assert_eq!(fs::read(&path).unwrap(), bytes);
    }
    // Read-only inspection does not create an implicit painting session.
    assert_eq!(mcp.call("inspect_document", json!({}))["isError"], true);
}
#[test]
fn readonly_errors_limits_and_hash_guards_leave_dirty_painting_history_unchanged() {
    let mut mcp = client();
    mcp.ok("create_canvas", json!({"width":3,"height":2}));
    mcp.ok("paint_stroke",json!({"points":[{"x":0,"y":0},{"x":2,"y":1}],"color":"#123456FF","size":1,"brush":"round"}));
    let before = mcp.ok("inspect_document", json!({}));
    let path = fixtures().join("raster-metadata-fixtures/rgba8-icc-precedence.png");
    let report = mcp.ok("inspect_source", json!({"path":path}));
    assert_common(&report, &fs::read(&path).unwrap(), "png");
    for args in [
        json!({"path":path,"expect_source_sha256":"0".repeat(64)}),
        json!({"path":path,"max_input_bytes":1}),
        json!({"path":path,"max_pixels":1}),
        json!({"path":path,"max_metadata_bytes":1}),
        json!({"path":fixtures().join("tiff-orientation-fixtures/rgba8-le-orientation-1.tif")}),
        json!({"path":path,"unknown_option":true}),
    ] {
        let result = mcp.call("inspect_source", args);
        assert_eq!(result["isError"], true, "{result}");
        assert_eq!(mcp.ok("inspect_document", json!({})), before);
    }
    for extra in [
        vec!["--max-input-bytes", "1"],
        vec!["--max-pixels", "1"],
        vec!["--max-metadata-bytes", "1"],
    ] {
        let result = cli(&path, &extra);
        assert_eq!(result.status.code(), Some(5));
        let body: Value = serde_json::from_slice(&result.stdout).unwrap();
        assert_eq!(body["code"], "resource.limit");
        assert_eq!(body["committed"], false);
    }
    let maximum = value(cli(
        &path,
        &["--max-metadata-bytes", "18446744073709551615"],
    ));
    assert!(maximum["limits"]["metadata_bytes"].as_u64().is_some());
    assert!(maximum["limits"]["metadata_bytes"].as_u64().unwrap() <= 1 << 31);
    mcp.ok("history", json!({}));
    assert_ne!(
        mcp.ok("inspect_document", json!({}))["state_id"],
        before["state_id"]
    );
    mcp.ok("history", json!({"redo":true}));
    assert_eq!(mcp.ok("inspect_document", json!({})), before);
    let unchanged = Command::new(env!("CARGO_BIN_EXE_paintcraft-cli"))
        .arg("inspect")
        .arg(&path)
        .arg("--json")
        .output()
        .unwrap();
    assert!(
        !unchanged.status.success(),
        "existing PCND inspect is not silently changed into source inspection"
    );
}

#[test]
fn published_schema_is_generated_from_the_actual_three_format_report_types() {
    let output = Command::new(env!("CARGO_BIN_EXE_paintcraft-cli"))
        .args(["schema", "source-inspection", "--json"])
        .output()
        .unwrap();
    let schema = value(output);
    assert!(
        schema["$schema"]
            .as_str()
            .unwrap()
            .contains("json-schema.org")
    );
    let definitions = schema["$defs"].as_object().unwrap();
    for name in ["Png", "Bmp", "Jpeg"] {
        let facts = &definitions[name];
        assert!(facts["properties"]["width"].is_object());
        assert!(facts["properties"]["height"].is_object());
        assert!(
            facts["required"]
                .as_array()
                .unwrap()
                .contains(&json!("width"))
        );
    }
    assert!(definitions["Bmp"]["properties"]["component_depths"].is_object());
    assert!(definitions["Png"]["properties"]["exif_historical_payload"].is_object());
    assert!(definitions["Jpeg"]["properties"]["components"].is_object());
    assert!(schema["properties"]["inspection_only"].is_object());
    assert!(schema["properties"].get("ticket").is_none());
}
