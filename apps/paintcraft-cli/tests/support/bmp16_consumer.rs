//! Real CLI and stdio MCP conversion, independent rows and cross-transport PCND.
use super::*;

#[test]
fn bmp16_actual_cli_stdio_imports_keep_exact_rows_ids_and_saved_projects() {
    let folder = Folder::new();
    let root = fixtures().with_file_name("bmp16-fixtures");
    let oracle: Value =
        serde_json::from_slice(&fs::read(root.join("oracle.json")).unwrap()).unwrap();
    for row in oracle["fixtures"].as_array().unwrap() {
        let name = row["file"].as_str().unwrap().to_owned();
        let bytes = fs::read(root.join(&name)).unwrap();
        let pixels = fs::read(root.join(name.replace(".bmp", ".rgba"))).unwrap();
        assert_eq!(
            paintcraft_document::hex(&Sha256::digest(&bytes)),
            row["sha256"].as_str().unwrap()
        );
        assert_eq!(
            paintcraft_document::hex(&Sha256::digest(&pixels)),
            row["expected_sha256"].as_str().unwrap()
        );
        let expected = retained_expected(&bytes, (65, 3), &pixels);
        let case = Case {
            name,
            bytes,
            pixels,
            size: (65, 3),
            expected,
        };
        let source = folder.0.join(&case.name);
        fs::write(&source, &case.bytes).unwrap();
        let output = folder.0.join(format!("cli-{}.pcnd", case.name));
        let losses = case.retained_losses();
        let result = cli(&source, &output, &losses, "studio");
        assert!(
            result.status.success(),
            "{} {}",
            String::from_utf8_lossy(&result.stdout),
            String::from_utf8_lossy(&result.stderr)
        );
        let a = read(&output);
        case.assert_doc(&a);
        let mut client = mcp_client::Client::at(Path::new(env!("CARGO_BIN_EXE_paintcraft-cli")));
        client.ok("import_image", json!({"path":source,"allow_losses":losses}));
        let saved = folder.0.join(format!("mcp-{}.pcnd", case.name));
        client.ok("save_project", json!({"path":saved}));
        let b = read(&saved);
        case.assert_doc(&b);
        assert_ids(&a, &b);
        assert_eq!(fs::read(&source).unwrap(), case.bytes);
        if let Some(root) = std::env::var_os("PAINTCRAFT_BMP16_CONSUMER_EVIDENCE") {
            let root = PathBuf::from(root);
            for transport in ["native", "web"] {
                let peer = root.join(transport).join(format!("{}.pcnd", case.name));
                case.assert_doc(&read(&peer));
                assert_eq!(fs::read(&peer).unwrap(), fs::read(&output).unwrap());
            }
            assert_eq!(fs::read(&saved).unwrap(), fs::read(&output).unwrap());
            for (transport, path) in [("cli", &output), ("mcp", &saved)] {
                let target = root.join(transport);
                fs::create_dir_all(&target).unwrap();
                fs::copy(path, target.join(format!("{}.pcnd", case.name))).unwrap();
            }
        }
    }
}
