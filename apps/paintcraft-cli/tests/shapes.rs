//! Real shape-resource CLI IO, preserving complete integer path semantics.
use paintcraft_formats::csh;
use serde_json::{Value, json};
use std::{
    fs,
    process::{Command, Output},
};
const PATHS: &[u8] =
    include_bytes!("../../../crates/services/paintcraft-formats/tests/csh-fixtures/paths.csh");
fn run(args: &[&str]) -> Output {
    Command::new(env!("CARGO_BIN_EXE_paintcraft-cli"))
        .args(args)
        .output()
        .unwrap()
}
fn object(output: Output) -> Value {
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    serde_json::from_slice(&output.stdout).unwrap()
}
#[test]
fn shape_inspection_and_atomic_copy_preserve_every_record_and_guard_sources() {
    let root = std::env::temp_dir().join(format!("paintcraft-shapes-cli-{}", std::process::id()));
    fs::create_dir_all(&root).unwrap();
    let input = root.join("source.csh");
    let output = root.join("copy.csh");
    fs::write(&input, PATHS).unwrap();
    let source = input.to_str().unwrap();
    let dest = output.to_str().unwrap();
    let inspected = object(run(&["shape", "inspect", source, "--json"]));
    assert_eq!(inspected["schema"], "paintcraft.shape-inspection/v1");
    assert_eq!(inspected["resource_type"], "shape-library");
    assert_eq!(
        inspected["coordinate_encoding"],
        "signed-normalized-8.24-i32"
    );
    let shapes = inspected["shapes"].as_array().unwrap();
    assert_eq!(shapes.len(), 5);
    assert_eq!(shapes[0]["id"], shapes[1]["id"]);
    assert_eq!(shapes[0]["name"], "形 / Shape 🚀");
    for (index, operation) in [
        Value::Null,
        json!("exclude"),
        json!("combine"),
        json!("subtract"),
        json!("intersect"),
    ]
    .into_iter()
    .enumerate()
    {
        let shape = &shapes[index];
        assert_eq!(
            shape["bounds_tlbr"],
            json!([10 + index, 20 + index, 110 + 2 * index, 220 + 2 * index])
        );
        let records = shape["records"].as_array().unwrap();
        assert_eq!(records.len(), 9);
        assert_eq!(records[0], json!({"kind":"fill-rule","rule":0}));
        assert_eq!(
            records[1],
            json!({"kind":"initial-fill","filled":index%2!=0})
        );
        assert_eq!(
            records[2],
            json!({"kind":"clipboard","values_i32":[0,4194304,16777216,16777216,1207959552]})
        );
        for (start, open) in [(3, false), (6, true)] {
            assert_eq!(
                records[start],
                json!({"kind":"subpath","open":open,"knots":2,"operation":operation,"fill_rule":(["even-odd-legacy","even-odd","non-zero"][index%3])})
            );
            for (offset, linked) in [(1, true), (2, false)] {
                assert_eq!(
                    records[start + offset],
                    json!({"kind":"knot","open":open,"linked":linked,"points_i32":[16777216,-1,1,8388608,0,-16777216]})
                );
            }
        }
    }
    fs::write(&output, b"existing destination").unwrap();
    assert_eq!(
        run(&["shape", "export", source, "-o", dest]).status.code(),
        Some(6)
    );
    assert_eq!(fs::read(&output).unwrap(), b"existing destination");
    let receipt = object(run(&[
        "shape",
        "export",
        source,
        "-o",
        dest,
        "--overwrite",
        "--json",
    ]));
    assert_eq!(receipt["engine"], inspected);
    assert_eq!(receipt["file"]["operation"], "export-asset");
    assert!(receipt["file"]["acknowledged_state_id"].is_null());
    assert_eq!(receipt["file"]["source_lineage_adopted"], false);
    let expected = csh::decode(PATHS, &|| Ok(())).unwrap();
    let bytes = fs::read(&output).unwrap();
    assert_eq!(csh::decode(&bytes, &|| Ok(())).unwrap(), expected);
    for args in [
        vec!["shape", "export", source, "-o", source, "--overwrite"],
        vec![
            "shape",
            "export",
            source,
            "-o",
            dest,
            "--overwrite",
            "--expect-input-sha256",
            "0000000000000000000000000000000000000000000000000000000000000000",
        ],
    ] {
        assert!(!run(&args).status.success());
        assert_eq!(fs::read(&input).unwrap(), PATHS);
        assert_eq!(fs::read(&output).unwrap(), bytes);
    }
    let alias = root.join("alias.csh");
    fs::hard_link(&input, &alias).unwrap();
    assert_eq!(
        run(&[
            "shape",
            "export",
            source,
            "-o",
            alias.to_str().unwrap(),
            "--overwrite"
        ])
        .status
        .code(),
        Some(2)
    );
    assert_eq!(fs::read(alias).unwrap(), PATHS);
    let invalid = root.join("invalid.csh");
    fs::write(&invalid, b"cush").unwrap();
    assert!(
        !run(&[
            "shape",
            "export",
            invalid.to_str().unwrap(),
            "-o",
            dest,
            "--overwrite"
        ])
        .status
        .success()
    );
    assert_eq!(fs::read(output).unwrap(), bytes);
    assert_eq!(fs::read(input).unwrap(), PATHS);
    fs::remove_dir_all(root).unwrap();
}
#[test]
fn studio_asset_policy_and_schema_are_explicit_without_image_routes() {
    for profile in ["classic-v7", "classic-v10", "classic-v11"] {
        assert_eq!(
            run(&[
                "shape",
                "inspect",
                "not-acquired.csh",
                "--frontend",
                profile
            ])
            .status
            .code(),
            Some(3)
        );
        assert_eq!(
            run(&[
                "shape",
                "export",
                "not-acquired.csh",
                "-o",
                "not-created.csh",
                "--frontend",
                profile
            ])
            .status
            .code(),
            Some(3)
        );
    }
    assert_eq!(
        run(&["shape", "inspect", "not-acquired.ase"]).status.code(),
        Some(2)
    );
    let schema = object(run(&["schema", "shape-inspection"]));
    assert_eq!(schema["$id"], "urn:paintcraft:schema:shape-inspection-v1");
    assert_eq!(
        schema["$defs"]["Record"]["oneOf"].as_array().unwrap().len(),
        5
    );
    let text = schema.to_string();
    for key in [
        "bounds_tlbr",
        "points_i32",
        "values_i32",
        "operation",
        "fill_rule",
        "linked",
        "open",
        "initial-fill",
        "clipboard",
    ] {
        assert!(text.contains(key), "{key}");
    }
    let capabilities = object(run(&["capabilities", "--json"]));
    let rows = capabilities["capabilities"].as_array().unwrap();
    for id in ["csh.shapes.read", "csh.shapes.write"] {
        let row = rows.iter().find(|row| row["capability"] == id).unwrap();
        assert_eq!(row["release_qualified"], false);
    }
}
