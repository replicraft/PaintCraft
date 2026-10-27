//! Actual render/export_image routes; optional retained files feed Pillow checks.
use paintcraft_core::types::Rgba;
use paintcraft_document::{POLICY, new_document};
use paintcraft_formats::{export, image::ImageFormat, pcnd};
use serde_json::{Value, json};
use std::{
    fs,
    path::{Path, PathBuf},
    process::Command,
};
#[path = "support/mcp_client.rs"]
mod mcp_client;
use mcp_client::Client;
fn root(label: &str) -> PathBuf {
    let root = std::env::var_os("PAINTCRAFT_DDS_CONSUMER_OUTPUT")
        .map(PathBuf::from)
        .unwrap_or_else(std::env::temp_dir)
        .join(format!("{label}-{}", uuid::Uuid::new_v4()));
    fs::create_dir(&root).unwrap();
    root
}
fn run(args: &[&str]) -> std::process::Output {
    Command::new(env!("CARGO_BIN_EXE_paintcraft-cli"))
        .args(args)
        .output()
        .unwrap()
}
fn success(args: &[&str]) -> Value {
    let result = run(args);
    assert!(
        result.status.success(),
        "{} {}",
        String::from_utf8_lossy(&result.stdout),
        String::from_utf8_lossy(&result.stderr)
    );
    serde_json::from_slice(&result.stdout).unwrap()
}
fn u32_at(bytes: &[u8], offset: usize) -> u32 {
    u32::from_le_bytes(bytes[offset..offset + 4].try_into().unwrap())
}
fn header(path: &Path, encoding: &str, linear: bool) {
    let bytes = fs::read(path).unwrap();
    assert_eq!(&bytes[..4], b"DDS ");
    assert_eq!(
        (u32_at(&bytes, 16), u32_at(&bytes, 12), u32_at(&bytes, 28)),
        (8, 4, 4)
    );
    let dxgi = match encoding {
        "rgba8" => 28,
        "bc1" => 71,
        "bc2" => 74,
        "bc3" => 77,
        _ => unreachable!(),
    };
    assert_eq!(u32_at(&bytes, 128), dxgi + u32::from(!linear));
    assert_eq!(
        (
            u32_at(&bytes, 132),
            u32_at(&bytes, 136),
            u32_at(&bytes, 140)
        ),
        (3, 0, 1)
    );
    assert_eq!(u32_at(&bytes, 144), if encoding == "bc1" { 3 } else { 1 });
    let mut size = 148usize;
    for (w, h) in [(8usize, 4usize), (4, 2), (2, 1), (1, 1)] {
        size += if encoding == "rgba8" {
            w * h * 4
        } else {
            w.div_ceil(4) * h.div_ceil(4) * if encoding == "bc1" { 8 } else { 16 }
        };
    }
    assert_eq!(bytes.len(), size);
}
#[test]
fn actual_cli_defaults_explicit_profiles_and_source_guards() {
    let root = root("cli");
    let source = root.join("source.pcnd");
    let doc = new_document(8, 4, Some(Rgba([255, 0, 255, 255])), 0, POLICY).unwrap();
    let input = pcnd::write(&doc).unwrap();
    fs::write(&source, &input).unwrap();
    let path = |p: &Path| p.to_str().unwrap().to_owned();
    let source_text = path(&source);
    let old = root.join("old.dds");
    success(&["render", &source_text, "--output", &path(&old), "--json"]);
    assert_eq!(
        fs::read(&old).unwrap(),
        export::export_flat_options(&doc, export::FlatOptions::new(ImageFormat::Dds, 90), &[])
            .unwrap()
            .bytes
    );
    for encoding in ["rgba8", "bc1", "bc2", "bc3"] {
        for transfer in ["srgb", "linear8"] {
            for filter in ["nearest", "box-stored"] {
                let dest = root.join(format!("magenta-{encoding}-{transfer}-{filter}.dds"));
                let mut args = vec![
                    "render",
                    &source_text,
                    "--output",
                    dest.to_str().unwrap(),
                    "--dds-encoding",
                    encoding,
                    "--dds-transfer",
                    transfer,
                    "--dds-mips",
                    "full",
                    "--dds-filter",
                    filter,
                    "--json",
                ];
                if encoding != "rgba8" {
                    args.extend([
                        "--dds-effort",
                        "high",
                        "--allow-loss",
                        "dds-block-compression",
                    ]);
                }
                let result = success(&args);
                assert!(result.to_string().contains("native-dds-texture-v1"));
                header(&dest, encoding, transfer == "linear8");
            }
        }
    }
    for extra in [
        vec!["--dds-encoding", "bc3"],
        vec!["--dds-levels", "5"],
        vec!["--dds-mips", "full", "--dds-levels", "2"],
        vec!["--dds-effort", "high"],
        vec!["--dds-transfer", "srgb", "--render-workers", "2"],
        vec![
            "--dds-transfer",
            "srgb",
            "--expect-input-sha256",
            &"0".repeat(64),
        ],
        vec!["--dds-transfer", "srgb", "--frontend", "classic-v11"],
    ] {
        let output = root.join("denied.dds");
        let mut args = vec![
            "render",
            &source_text,
            "--output",
            output.to_str().unwrap(),
            "--json",
        ];
        args.extend(extra);
        assert!(!run(&args).status.success());
        assert!(!output.exists());
    }
    let wrong = root.join("wrong.png");
    assert!(
        !run(&[
            "render",
            &source_text,
            "--output",
            wrong.to_str().unwrap(),
            "--dds-mips",
            "full",
            "--json"
        ])
        .status
        .success()
    );
    let alias = root.join("source-alias.dds");
    fs::hard_link(&source, &alias).unwrap();
    assert!(
        !run(&[
            "render",
            &source_text,
            "--output",
            alias.to_str().unwrap(),
            "--overwrite",
            "--dds-transfer",
            "srgb",
            "--json"
        ])
        .status
        .success()
    );
    assert_eq!(fs::read(&source).unwrap(), input);
    if std::env::var_os("PAINTCRAFT_DDS_CONSUMER_OUTPUT").is_none() {
        fs::remove_dir_all(root).unwrap();
    }
}
#[test]
fn actual_mcp_linear_conversion_consents_renamed_source_and_state() {
    let root = root("mcp");
    let mut client = Client::at(Path::new(env!("CARGO_BIN_EXE_paintcraft-cli")));
    client.ok(
        "create_canvas",
        json!({"width":8,"height":4,"background":"#80BCFFFF"}),
    );
    let source = root.join("source.pcnd");
    client.ok("save_project", json!({"path":source}));
    client.ok("open_project", json!({"path":source}));
    let moved = root.join("renamed.pcnd");
    fs::rename(&source, &moved).unwrap();
    let before = client.ok("inspect_document", json!({}));
    let path = root.join("linear-rgba8-linear8-nearest.dds");
    let options = json!({"encoding":"rgba8","transfer":"linear8","mips":"full"});
    assert_eq!(
        client.call("export_image", json!({"path":path,"dds":options}))["isError"],
        true
    );
    assert!(!path.exists());
    let receipt = client.ok(
        "export_image",
        json!({"path":path,"dds":options,"allow_losses":["dds-srgb-linear8-quantization"]}),
    );
    assert_eq!(receipt["export"]["quantized_rgb_samples"], 64);
    assert_eq!(
        receipt["export"]["transfer_policy"],
        "srgb8-to-linear8-nearest-ties-up-v1"
    );
    header(&path, "rgba8", true);
    let bytes = fs::read(&path).unwrap();
    assert_eq!(&bytes[148..148 + 128], &[55, 128, 255, 255].repeat(32));
    let alias = root.join("source-alias.dds");
    fs::hard_link(&moved, &alias).unwrap();
    let original = fs::read(&moved).unwrap();
    assert_eq!(
        client.call(
            "export_image",
            json!({"path":alias,"overwrite":true,"dds":{}})
        )["isError"],
        true
    );
    assert_eq!(fs::read(&alias).unwrap(), original);
    client.ok("export_image",json!({"path":path,"overwrite":true,"dds":options,"allow_losses":["dds-srgb-linear8-quantization"]}));
    for (name, opts) in [
        ("wrong.png", json!({})),
        ("bad.dds", json!({"levels":0})),
        ("unknown.dds", json!({"unexpected":true})),
    ] {
        let failed = client.call("export_image", json!({"path":root.join(name),"dds":opts}));
        assert_eq!(failed["isError"], true, "{failed}");
        assert!(!root.join(name).exists());
    }
    assert_eq!(client.ok("inspect_document", json!({})), before);
    assert_eq!(fs::read(&moved).unwrap(), original);
    if std::env::var_os("PAINTCRAFT_DDS_CONSUMER_OUTPUT").is_none() {
        fs::remove_dir_all(root).unwrap();
    }
}

#[test]
fn actual_cli_nonuniform_box_normal_matches_independent_whole_file_golden() {
    const SOURCE: &[u8] = include_bytes!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../crates/services/paintcraft-formats/tests/dds-texture-write-fixtures/sources/7x5-opaque.rgba"
    ));
    const BOX: &[u8] = include_bytes!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../crates/services/paintcraft-formats/tests/dds-texture-write-fixtures/normal/7x5-Bc1-Srgb-BoxStored.dds"
    ));
    const NEAREST: &[u8] = include_bytes!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../crates/services/paintcraft-formats/tests/dds-texture-write-fixtures/normal/7x5-Bc1-Srgb-Nearest.dds"
    ));
    assert_ne!(
        BOX, NEAREST,
        "The independent goldens must distinguish mip filters"
    );
    assert_eq!(SOURCE.len(), 7 * 5 * 4);
    assert!(
        SOURCE
            .as_chunks::<4>()
            .0
            .iter()
            .all(|pixel| pixel[3] == 255)
    );
    let mut doc = new_document(7, 5, Some(Rgba([0, 0, 0, 255])), 0, POLICY).unwrap();
    doc.layers.get_mut(&doc.roots[0]).unwrap().raster = paintcraft_raster::RasterLayer::from_rgba(
        7,
        5,
        Rgba([0, 0, 0, 0]),
        SOURCE,
        &mut paintcraft_raster::Admission::new(65536),
    )
    .unwrap();
    assert_eq!(paintcraft_render::render_flat(&doc).unwrap(), SOURCE);
    let root = root("nonuniform");
    let source = root.join("source.pcnd");
    let original = pcnd::write(&doc).unwrap();
    fs::write(&source, &original).unwrap();
    let output = root.join("7x5-Bc1-Srgb-BoxStored.dds");
    success(&[
        "render",
        source.to_str().unwrap(),
        "--output",
        output.to_str().unwrap(),
        "--dds-encoding",
        "bc1",
        "--dds-transfer",
        "srgb",
        "--dds-mips",
        "full",
        "--dds-filter",
        "box-stored",
        "--dds-effort",
        "normal",
        "--allow-loss",
        "dds-block-compression",
        "--json",
    ]);
    assert_eq!(fs::read(&output).unwrap(), BOX);
    assert_eq!(fs::read(&source).unwrap(), original);
    if std::env::var_os("PAINTCRAFT_DDS_CONSUMER_OUTPUT").is_none() {
        fs::remove_dir_all(root).unwrap();
    }
}
