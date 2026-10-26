use paintcraft_core::types::json::{self, Value};
use paintcraft_formats::ase;
use std::{
    fs,
    process::{Command, Output},
};
fn run(args: &[&str]) -> Output {
    Command::new(env!("CARGO_BIN_EXE_paintcraft-cli"))
        .args(args)
        .output()
        .unwrap()
}
fn object(output: &Output) -> Value {
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    json::parse(&output.stdout).unwrap()
}
const MIXED: &[u8] =
    include_bytes!("../../../crates/services/paintcraft-formats/tests/ase-fixtures/mixed.ase");
#[test]
fn palette_inspection_and_fresh_export_preserve_asset_semantics_and_file_receipts() {
    let root = std::env::temp_dir().join(format!("paintcraft-palette-cli-{}", std::process::id()));
    fs::create_dir_all(&root).unwrap();
    let source = root.join("source.ase");
    let output = root.join("copy.ase");
    fs::write(&source, MIXED).unwrap();
    let input = source.to_str().unwrap();
    let dest = output.to_str().unwrap();
    let inspected = object(&run(&["palette", "inspect", input, "--json"]));
    assert_eq!(
        inspected.get("schema").and_then(Value::as_str),
        Some("paintcraft.palette-inspection/v1")
    );
    assert_eq!(
        inspected.get("resource_type").and_then(Value::as_str),
        Some("palette")
    );
    assert_eq!(inspected.get("swatches").and_then(Value::as_i64), Some(6));
    let entries = inspected.get("entries").unwrap().as_array().unwrap();
    assert_eq!(
        entries[1].get("group").and_then(Value::as_str),
        Some("印刷 / Print")
    );
    assert_eq!(
        entries[3]
            .get("swatch")
            .unwrap()
            .get("channels_f32_be")
            .unwrap()
            .as_array()
            .unwrap()[0]
            .as_str(),
        Some("80000000")
    );
    fs::write(&output, b"existing destination").unwrap();
    assert_eq!(
        run(&["palette", "export", input, "-o", dest]).status.code(),
        Some(6)
    );
    assert_eq!(fs::read(&output).unwrap(), b"existing destination");
    let result = object(&run(&[
        "palette",
        "export",
        input,
        "-o",
        dest,
        "--overwrite",
        "--json",
    ]));
    let receipt = result.get("file").unwrap();
    assert_eq!(
        receipt.get("operation").and_then(Value::as_str),
        Some("export-asset")
    );
    assert_eq!(
        receipt.get("replacement").and_then(Value::as_str),
        Some("replaced")
    );
    assert_eq!(receipt.get("acknowledged_state_id"), Some(&Value::Null));
    assert_eq!(
        receipt.get("source_lineage_adopted"),
        Some(&Value::Bool(false))
    );
    assert_eq!(result.get("engine"), Some(&inspected));
    assert_eq!(fs::read(&output).unwrap(), MIXED);
    assert_eq!(
        ase::decode(&fs::read(&output).unwrap(), &|| Ok(()))
            .unwrap()
            .swatch_count(),
        6
    );
    assert_eq!(fs::read(&source).unwrap(), MIXED);
    for args in [
        vec!["palette", "export", input, "-o", input, "--overwrite"],
        vec![
            "palette",
            "export",
            input,
            "-o",
            dest,
            "--overwrite",
            "--expect-input-sha256",
            "0000000000000000000000000000000000000000000000000000000000000000",
        ],
    ] {
        assert!(!run(&args).status.success());
        assert_eq!(fs::read(&source).unwrap(), MIXED);
        assert_eq!(fs::read(&output).unwrap(), MIXED);
    }
    #[cfg(unix)]
    {
        let alias = root.join("hardlink.ase");
        fs::hard_link(&source, &alias).unwrap();
        assert_eq!(
            run(&[
                "palette",
                "export",
                input,
                "-o",
                alias.to_str().unwrap(),
                "--overwrite"
            ])
            .status
            .code(),
            Some(2)
        );
        assert_eq!(fs::read(&source).unwrap(), MIXED);
    }
    fs::remove_dir_all(&root).unwrap();
}
#[test]
fn palette_policy_precedes_acquisition_and_does_not_leak_into_document_import() {
    for profile in ["classic-v7", "classic-v10", "classic-v11"] {
        assert_eq!(
            run(&[
                "palette",
                "inspect",
                "not-acquired.ase",
                "--frontend",
                profile
            ])
            .status
            .code(),
            Some(3)
        );
        assert_eq!(
            run(&[
                "palette",
                "export",
                "not-acquired.ase",
                "-o",
                "not-created.ase",
                "--frontend",
                profile
            ])
            .status
            .code(),
            Some(3)
        );
    }
    assert_eq!(
        run(&["palette", "inspect", "not-acquired.aco"])
            .status
            .code(),
        Some(2)
    );
    assert_eq!(
        run(&[
            "palette",
            "export",
            "not-acquired.ase",
            "-o",
            "not-created.png"
        ])
        .status
        .code(),
        Some(2)
    );
    let root = std::env::temp_dir().join(format!("paintcraft-palette-bad-{}", std::process::id()));
    fs::create_dir_all(&root).unwrap();
    let source = root.join("broken.ase");
    let output = root.join("untouched.ase");
    fs::write(&source, &MIXED[..MIXED.len() - 1]).unwrap();
    fs::write(&output, b"old").unwrap();
    assert_eq!(
        run(&[
            "palette",
            "export",
            source.to_str().unwrap(),
            "-o",
            output.to_str().unwrap(),
            "--overwrite"
        ])
        .status
        .code(),
        Some(6)
    );
    assert_eq!(fs::read(&output).unwrap(), b"old");
    fs::write(&source, MIXED).unwrap();
    assert!(
        !run(&[
            "import",
            source.to_str().unwrap(),
            "-o",
            root.join("wrong.pcnd").to_str().unwrap()
        ])
        .status
        .success()
    );
    assert!(!root.join("wrong.pcnd").exists());
    let oversized = fs::File::create(&source).unwrap();
    oversized.set_len(ase::BYTES_MAX as u64 + 1).unwrap();
    drop(oversized);
    assert_eq!(
        run(&["palette", "inspect", source.to_str().unwrap()])
            .status
            .code(),
        Some(5)
    );
    assert!(
        !run(&["palette", "inspect", root.to_str().unwrap()])
            .status
            .success()
    );
    fs::remove_dir_all(root).unwrap();
}
