//! Actual complete installed CLI/MCP copies and independent whole-file JXR pixels.
use super::mcp_client::Client;
use super::*;
use paintcraft_core::types::Rgba;
use paintcraft_document::{POLICY, new_document};
use paintcraft_formats::{codec_worker::jxr_encode as encode, pcnd};
use paintcraft_raster::{Admission, RasterLayer};
use serde_json::{Value as Json, json};
fn installation() -> Installation {
    Installation::from_installed(&PathBuf::from(
        std::env::var_os("PAINTCRAFT_JXR_INSTALLATION").expect("complete final JXR installation"),
    ))
}
fn project(width: u32, height: u32) -> Vec<u8> {
    let mut doc = new_document(width, height, Some(Rgba([0, 0, 0, 255])), 0, POLICY).unwrap();
    let mut bytes = Vec::new();
    for y in 0..height {
        for x in 0..width {
            bytes.extend([
                ((x * 47 + y * 13) % 256) as u8,
                ((x * 47 + y * 13 + 71) % 256) as u8,
                ((x * 47 + y * 13 + 142) % 256) as u8,
                255,
            ]);
        }
    }
    doc.layers.get_mut(&doc.roots[0]).unwrap().raster = RasterLayer::from_rgba(
        width,
        height,
        Rgba([0, 0, 0, 255]),
        &bytes,
        &mut Admission::new(8 << 20),
    )
    .unwrap();
    pcnd::write(&doc).unwrap()
}
fn execute(installation: &Installation, args: &[&str]) -> (i32, Json) {
    let output = Command::new(&installation.cli)
        .arg("--json")
        .args(args)
        .output()
        .unwrap();
    let value = serde_json::from_slice(&output.stdout).unwrap_or_else(|_| {
        panic!(
            "{} {}",
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr)
        )
    });
    (output.status.code().unwrap(), value)
}
fn independent_pixels(path: &Path, width: u32, height: u32) {
    let oracle =
        PathBuf::from(std::env::var_os("PAINTCRAFT_JXR_REFERENCE").expect("pinned C oracle"));
    assert_eq!(
        hex(&Sha256::digest(fs::read(&oracle).unwrap())),
        "8e3b71db2c53c5db5f988b8755946a30ac1070b7a6c0a8ffa6933d34d26b95ce"
    );
    let output = path.with_extension("C.rgb");
    let args = [
        std::ffi::OsString::from("decode"),
        width.to_string().into(),
        height.to_string().into(),
        path.as_os_str().to_owned(),
        output.as_os_str().to_owned(),
    ];
    let result = paintcraft_runtime::process::run(
        &oracle,
        &args,
        vec![],
        paintcraft_runtime::process::Limits {
            input_bytes: 1,
            output_bytes: 4096,
            deadline: std::time::Duration::from_secs(10),
            resource_exit_code: None,
        },
        &|| Ok(()),
    )
    .unwrap();
    assert!(result.status.success());
    let expected: Vec<_> = (0..height)
        .flat_map(|y| {
            (0..width).flat_map(move |x| {
                [
                    ((x * 47 + y * 13) % 256) as u8,
                    ((x * 47 + y * 13 + 71) % 256) as u8,
                    ((x * 47 + y * 13 + 142) % 256) as u8,
                ]
            })
        })
        .collect();
    assert_eq!(fs::read(output).unwrap(), expected);
}
#[test]
#[ignore = "requires complete nonprivate JXR installation and pinned C oracle"]
fn installed_cli_fresh_copies_source_guards_and_own_output_read_are_real() {
    let installation = installation();
    for (w, h) in [(4, 3), (17, 19), (4096, 1), (1, 4096)] {
        let bytes = project(w, h);
        let input = installation.fixture(&format!("{w}x{h}.pcnd"), &bytes);
        let dest = installation.root.join(format!("{w}x{h}.jxr"));
        let (code, receipt) = execute(
            &installation,
            &[
                "export-jxr",
                input.to_str().unwrap(),
                "--output",
                dest.to_str().unwrap(),
            ],
        );
        assert_eq!(code, 0, "{receipt}");
        assert_eq!(receipt["outcome"], "success");
        independent_pixels(&dest, w, h);
        assert_eq!(fs::read(&input).unwrap(), bytes);
        let reopened = installation.root.join(format!("reopened-{w}x{h}.pcnd"));
        let (code, receipt) = execute(
            &installation,
            &[
                "import",
                dest.to_str().unwrap(),
                "--output",
                reopened.to_str().unwrap(),
                "--allow-loss",
                "jxr-resolution-not-retained",
            ],
        );
        assert_eq!(code, 0, "{receipt}");
        let doc = pcnd::read(&fs::read(reopened).unwrap()).unwrap();
        assert_eq!(doc.canvas, (w, h));
        assert_eq!(
            paintcraft_render::render_flat(&doc).unwrap(),
            paintcraft_render::render_flat(&pcnd::read(&bytes).unwrap()).unwrap()
        );
        if w > 4080 || h > 4080 {
            assert_eq!(
                receipt["engine"]["decoder_profile"],
                encode::qualified::WRITER_IMPORT_VARIANT
            );
        }
        let denied = installation.fixture("blocked.jxr", b"existing destination");
        let (code, _) = execute(
            &installation,
            &[
                "export-jxr",
                input.to_str().unwrap(),
                "--output",
                denied.to_str().unwrap(),
                "--overwrite",
                "--expect-input-sha256",
                &"0".repeat(64),
            ],
        );
        assert_ne!(code, 0);
        assert_eq!(fs::read(&denied).unwrap(), b"existing destination");
        let alias = installation.root.join(format!("alias-{w}x{h}.jxr"));
        fs::hard_link(&input, &alias).unwrap();
        let (code, _) = execute(
            &installation,
            &[
                "export-jxr",
                input.to_str().unwrap(),
                "--output",
                alias.to_str().unwrap(),
                "--overwrite",
            ],
        );
        assert_ne!(code, 0);
        assert_eq!(fs::read(&alias).unwrap(), bytes);
    }
    let source = installation.fixture("opaque.pcnd", &project(3, 2));
    let dest = installation.fixture("classic.jxr", b"preserve");
    let (code, _) = execute(
        &installation,
        &[
            "export-jxr",
            source.to_str().unwrap(),
            "--output",
            dest.to_str().unwrap(),
            "--overwrite",
            "--frontend",
            "classic-v11",
        ],
    );
    assert_ne!(code, 0);
    assert_eq!(fs::read(dest).unwrap(), b"preserve");
}
#[test]
#[ignore = "requires complete nonprivate JXR installation"]
fn installed_mcp_fresh_copy_preserves_dirty_native_binding_and_real_alias_guards() {
    let installation = installation();
    let source = installation.fixture("session.pcnd", &project(4096, 1));
    let mut client = Client::at(&installation.cli);
    client.ok("open_project", json!({"path":source}));
    client.ok(
        "apply_operations",
        json!({"operations":[{
            "op":"layer.add","ref":"overlay","name":"Overlay","width":4096,"height":1,
            "origin":{"x":0,"y":0},"default_pixel":"#00000000"
        }]}),
    );
    let before = client.ok("inspect_document", json!({}));
    assert_eq!(before["dirty"], true);
    let output = installation.root.join("session.jxr");
    assert_eq!(
        client.call("export_jxr", json!({"path":output}))["isError"],
        true
    );
    assert!(!output.exists());
    assert_eq!(client.ok("inspect_document", json!({})), before);
    let result = client.ok(
        "export_jxr",
        json!({"path":output,"allow_losses":["flatten"]}),
    );
    assert_eq!(result["save"]["replacement"], "replaced");
    assert_eq!(result["save"]["durability"], "confirmed");
    independent_pixels(&output, 4096, 1);
    assert_eq!(client.ok("inspect_document", json!({})), before);
    let renamed = installation.root.join("renamed.pcnd");
    fs::rename(&source, &renamed).unwrap();
    let unrelated = installation.root.join("after-rename.jxr");
    client.ok(
        "export_jxr",
        json!({"path":unrelated,"allow_losses":["flatten"]}),
    );
    let alias = installation.root.join("project-alias.jxr");
    fs::hard_link(&renamed, &alias).unwrap();
    let original = fs::read(&renamed).unwrap();
    assert_eq!(
        client.call(
            "export_jxr",
            json!({"path":alias,"overwrite":true,"allow_losses":["flatten"]})
        )["isError"],
        true
    );
    assert_eq!(fs::read(alias).unwrap(), original);
    let another = installation.root.join("after-alias.jxr");
    client.ok(
        "export_jxr",
        json!({"path":another,"allow_losses":["flatten"]}),
    );
    assert_eq!(client.ok("inspect_document", json!({})), before);
    let cancelled = installation.root.join("cancelled.jxr");
    client.cancel_call(
        "export_jxr",
        json!({"path":cancelled,"allow_losses":["flatten"]}),
    );
    assert_eq!(client.ok("inspect_document", json!({})), before);
    assert!(!cancelled.exists());
    let transparent = installation.root.join("transparent.jxr");
    client.ok(
        "create_canvas",
        json!({"width":3,"height":2,"background":"#1234567F","discard_changes":true}),
    );
    let state = client.ok("inspect_document", json!({}));
    assert_eq!(
        client.call("export_jxr", json!({"path":transparent}))["isError"],
        true
    );
    assert!(!transparent.exists());
    assert_eq!(client.ok("inspect_document", json!({})), state);
}
#[test]
#[ignore = "requires complete nonprivate JXR installation"]
fn installed_forged_export_contract_preserves_cli_destination_and_mcp_session() {
    let installation = installation();
    let source = installation.fixture("forged-source.pcnd", &project(3, 2));
    let directory = installation.root.join("forged-installation");
    fs::create_dir(&directory).unwrap();
    // macOS temp_dir may begin at /var, whose symlink is correctly refused by
    // production helper discovery. Exercise the malformed manifest on a real path.
    let directory = directory.canonicalize().unwrap();
    let cli = directory.join(installation.cli.file_name().unwrap());
    fs::copy(&installation.cli, &cli).unwrap();
    fs::copy(
        &installation.helper,
        directory.join(installation.helper.file_name().unwrap()),
    )
    .unwrap();
    let mut value: Json =
        serde_json::from_slice(&fs::read(&installation.manifest).unwrap()).unwrap();
    value["qualified_export_variants"] = json!([]);
    fs::write(
        directory.join("paintcraft-codec-worker.json"),
        serde_json::to_vec_pretty(&value).unwrap(),
    )
    .unwrap();
    // Preserve actual producer notices; only this private fixture manifest differs.
    for name in [
        "JPEGXR_LICENSE.txt",
        "DAV1D_LICENSE.txt",
        "HDR_EXR_LICENSES_V1.txt",
    ] {
        fs::copy(
            installation.cli.parent().unwrap().join(name),
            directory.join(name),
        )
        .unwrap();
    }
    let output = installation.fixture("forged-output.jxr", b"existing destination");
    let result = Command::new(&cli)
        .arg("--json")
        .arg("export-jxr")
        .arg(&source)
        .arg("--output")
        .arg(&output)
        .arg("--overwrite")
        .output()
        .unwrap();
    assert!(!result.status.success());
    let refusal: Json = serde_json::from_slice(&result.stdout).unwrap_or_else(|error| {
        panic!(
            "Invalid refusal {error}: status={} stdout={} stderr={}",
            result.status,
            String::from_utf8_lossy(&result.stdout),
            String::from_utf8_lossy(&result.stderr)
        )
    });
    eprintln!("forged contract CLI refusal: {refusal}");
    assert_eq!(refusal["file"]["replacement"], "not-replaced");
    assert!(
        refusal["file"]["reasons"]
            .as_array()
            .is_some_and(|reasons| reasons.iter().any(|reason| {
                reason.as_str().is_some_and(|text| {
                    text.contains("capability.unsupported") && text.contains("schema3")
                })
            })),
        "Expected exact helper contract refusal, got {refusal}"
    );
    assert_eq!(fs::read(&output).unwrap(), b"existing destination");
    let mut client = Client::at(&cli);
    client.ok("open_project", json!({"path":source}));
    let before = client.ok("inspect_document", json!({}));
    assert_eq!(
        client.call("export_jxr", json!({"path":output,"overwrite":true}))["isError"],
        true
    );
    assert_eq!(client.ok("inspect_document", json!({})), before);
    assert_eq!(fs::read(output).unwrap(), b"existing destination");
}
