//! Portable byte identity to separately C-qualified owned files; no C runs here.
use super::*;
const CASES: [(u32, u32); 4] = [(4, 3), (17, 19), (4096, 1), (1, 4096)];
fn rgba(w: u32, h: u32) -> Vec<u8> {
    (0..h)
        .flat_map(|y| {
            (0..w).flat_map(move |x| {
                [
                    ((x * 47 + y * 13) % 256) as u8,
                    ((x * 47 + y * 13 + 71) % 256) as u8,
                    ((x * 47 + y * 13 + 142) % 256) as u8,
                    255,
                ]
            })
        })
        .collect()
}
fn fixture_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/jxr-export-fixtures")
}
fn pixels(path: &Path, w: u32, h: u32, expected: &[u8]) {
    let doc = pcnd::read(&fs::read(path).unwrap()).unwrap();
    assert_eq!(doc.canvas, (w, h));
    assert_eq!(paintcraft_render::render_flat(&doc).unwrap(), expected);
}
#[test]
#[ignore = "one-shot golden curation: complete V8 installation, pinned C, fresh output directory"]
fn curate_c_qualified_goldens() {
    let installation = installation();
    let output = PathBuf::from(std::env::var_os("PAINTCRAFT_JXR_GOLDEN_OUTPUT").unwrap());
    fs::create_dir(&output).unwrap(); // Never overwrite an earlier proof.
    let mut cases = Vec::new();
    for (w, h) in CASES {
        let name = format!("{w}x{h}");
        let source = installation.fixture(&format!("{name}.pcnd"), &project(w, h));
        let encoded = output.join(format!("{name}.jxr"));
        let (status, receipt) = execute(
            &installation,
            &[
                "export-jxr",
                source.to_str().unwrap(),
                "--output",
                encoded.to_str().unwrap(),
            ],
        );
        assert_eq!(status, 0, "{receipt}");
        independent_pixels(&encoded, w, h);
        let raw = rgba(w, h);
        fs::write(output.join(format!("{name}.rgba")), &raw).unwrap();
        cases.push(json!({"name":name,"width":w,"height":h,
            "input_rgba_sha256":hex(&Sha256::digest(&raw)),
            "encoded_sha256":hex(&Sha256::digest(fs::read(&encoded).unwrap())),
            "reference_rgb_sha256":hex(&Sha256::digest(fs::read(encoded.with_extension("C.rgb")).unwrap())),
            "cli_receipt":receipt}));
    }
    let reference = PathBuf::from(std::env::var_os("PAINTCRAFT_JXR_REFERENCE").unwrap());
    let manifest = json!({"schema":1,"source_head":Command::new("git").args(["rev-parse","HEAD"]).output().map(|o|String::from_utf8(o.stdout).unwrap().trim().to_owned()).unwrap(),
        "cli_sha256":hex(&Sha256::digest(fs::read(&installation.cli).unwrap())),
        "helper_sha256":hex(&Sha256::digest(fs::read(&installation.helper).unwrap())),
        "reference_sha256":hex(&Sha256::digest(fs::read(reference).unwrap())),
        "cases":cases});
    fs::write(
        output.join("provenance.json"),
        serde_json::to_vec_pretty(&manifest).unwrap(),
    )
    .unwrap();
}
#[test]
#[ignore = "requires complete qualified public V8 CLI/helper installation; no C executable"]
fn installed_cli_mcp_match_c_qualified_goldens() {
    let installation = installation();
    let root = fixture_root();
    let manifest: Json =
        serde_json::from_slice(&fs::read(root.join("provenance.json")).unwrap()).unwrap();
    assert_eq!(manifest["schema"], 1);
    assert_eq!(
        manifest["reference_sha256"],
        "8e3b71db2c53c5db5f988b8755946a30ac1070b7a6c0a8ffa6933d34d26b95ce"
    );
    assert_eq!(manifest["cases"].as_array().unwrap().len(), CASES.len());
    for ((w, h), case) in CASES.into_iter().zip(manifest["cases"].as_array().unwrap()) {
        let name = format!("{w}x{h}");
        assert_eq!(case["name"], name);
        assert_eq!(case["width"], w);
        assert_eq!(case["height"], h);
        let expected = fs::read(root.join(format!("{name}.rgba"))).unwrap();
        assert_eq!(expected, rgba(w, h));
        assert_eq!(hex(&Sha256::digest(&expected)), case["input_rgba_sha256"]);
        let golden = fs::read(root.join(format!("{name}.jxr"))).unwrap();
        assert_eq!(hex(&Sha256::digest(&golden)), case["encoded_sha256"]);
        let rgb: Vec<_> = expected
            .as_chunks::<4>()
            .0
            .iter()
            .flat_map(|p| p[..3].iter().copied())
            .collect();
        assert_eq!(hex(&Sha256::digest(&rgb)), case["reference_rgb_sha256"]);
        let original = project(w, h);
        let source = installation.fixture(&format!("{name}.pcnd"), &original);
        let encoded = installation.root.join(format!("{name}.jxr"));
        let (status, receipt) = execute(
            &installation,
            &[
                "export-jxr",
                source.to_str().unwrap(),
                "--output",
                encoded.to_str().unwrap(),
            ],
        );
        assert_eq!(status, 0, "{receipt}");
        assert_eq!(fs::read(&encoded).unwrap(), golden);
        assert_eq!(fs::read(&source).unwrap(), original);
        let reopened = installation.root.join(format!("cli-{name}.pcnd"));
        let args = [
            "import",
            encoded.to_str().unwrap(),
            "--output",
            reopened.to_str().unwrap(),
        ];
        assert_ne!(execute(&installation, &args).0, 0);
        assert!(!reopened.exists());
        let (status, receipt) = execute(
            &installation,
            &[
                args.as_slice(),
                &["--allow-loss", "jxr-resolution-not-retained"],
            ]
            .concat(),
        );
        assert_eq!(status, 0, "{receipt}");
        pixels(&reopened, w, h, &expected);
        let blocked = installation.fixture(&format!("blocked-{name}.jxr"), b"preserve");
        assert_ne!(
            execute(
                &installation,
                &[
                    "export-jxr",
                    source.to_str().unwrap(),
                    "--output",
                    blocked.to_str().unwrap(),
                    "--overwrite",
                    "--expect-input-sha256",
                    &"0".repeat(64)
                ]
            )
            .0,
            0
        );
        assert_eq!(fs::read(&blocked).unwrap(), b"preserve");
        let mut client = Client::at(&installation.cli);
        client.ok("open_project", json!({"path":source}));
        client.ok("apply_operations", json!({"operations":[{"op":"layer.add","ref":"overlay","name":"Overlay","width":w,"height":h,"origin":{"x":0,"y":0},"default_pixel":"#00000000"}]}));
        let before = client.ok("inspect_document", json!({}));
        assert_eq!(before["dirty"], true);
        let exported = installation.root.join(format!("mcp-{name}.jxr"));
        assert_eq!(
            client.call("export_jxr", json!({"path":exported}))["isError"],
            true
        );
        assert!(!exported.exists());
        client.ok(
            "export_jxr",
            json!({"path":exported,"allow_losses":["flatten"]}),
        );
        assert_eq!(fs::read(&exported).unwrap(), golden);
        assert_eq!(client.ok("inspect_document", json!({})), before);
        // Source identity remains protected even after the original project is renamed.
        let renamed = installation.root.join(format!("renamed-{name}.pcnd"));
        fs::rename(&source, &renamed).unwrap();
        let alias = installation.root.join(format!("alias-{name}.jxr"));
        fs::hard_link(&renamed, &alias).unwrap();
        assert_eq!(
            client.call(
                "export_jxr",
                json!({"path":alias,"overwrite":true,"allow_losses":["flatten"]})
            )["isError"],
            true
        );
        assert_eq!(fs::read(&alias).unwrap(), original);
        assert_eq!(client.ok("inspect_document", json!({})), before);
        assert_eq!(
            client.call(
                "import_image",
                json!({"path":exported,"discard_changes":true})
            )["isError"],
            true
        );
        assert_eq!(client.ok("inspect_document", json!({})), before);
        client.ok("import_image", json!({"path":exported,"discard_changes":true,"allow_losses":["jxr-resolution-not-retained"]}));
        let saved = installation.root.join(format!("mcp-{name}.pcnd"));
        client.ok("save_project", json!({"path":saved}));
        pixels(&saved, w, h, &expected);
        let raw_alias = installation.root.join(format!("raw-alias-{name}.pcnd"));
        fs::hard_link(&exported, &raw_alias).unwrap();
        let stable = client.ok("inspect_document", json!({}));
        assert_eq!(
            client.call("save_project", json!({"path":raw_alias,"overwrite":true}))["isError"],
            true
        );
        assert_eq!(fs::read(raw_alias).unwrap(), golden);
        assert_eq!(client.ok("inspect_document", json!({})), stable);
    }
}
