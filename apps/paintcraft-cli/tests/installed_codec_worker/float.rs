//! Real CLI/stdio conversion against the same seventeen independent float cases.
use super::mcp_client::Client;
use super::*;
use serde_json::Value as Json;
fn installation() -> Installation {
    if let Some(directory) = std::env::var_os("PAINTCRAFT_V7_INSTALLATION") {
        return Installation::from_installed(&PathBuf::from(directory));
    }
    let helper = PathBuf::from(
        std::env::var_os("PAINTCRAFT_V7_HELPER").expect("final V7 helper or archive installation"),
    );
    let value = Installation::with_helper(Some(&helper));
    let metadata: Json = serde_json::from_slice(&fs::read(&value.manifest).unwrap()).unwrap();
    assert_eq!(metadata["profile"], wire::FLOAT_NATIVE_PROFILE);
    assert_eq!(
        metadata["qualified_import_variants"],
        serde_json::json!(wire::QUALIFIED_FLOAT_IMPORT_VARIANTS)
    );
    value
}
fn formats() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../crates/services/paintcraft-formats/tests")
}
fn source(case: &Json) -> PathBuf {
    let name = case["file"].as_str().unwrap();
    let group = if name == "aspect-2.hdr" {
        "float-import-fixtures"
    } else if case["family"] == "hdr" {
        "radiance-fixtures"
    } else {
        "exr-fixtures"
    };
    formats().join(group).join(name)
}
fn settings(case: &Json) -> Json {
    let value = &case["settings"];
    let assignment = |s: &str| match s {
        "rec709" => "linear-rec709-d65",
        "radiance" => "linear-radiance-nominal-ee",
        _ => panic!("space"),
    };
    serde_json::json!({"curve":if value["curve"]=="clip" {"clip"}else{"reinhard-per-channel"},
        "exposure_milli_ev":value["exposure_milli_ev"],"negative":if value["negative"]=="reject" {"reject"}else{"clip-to-black"},
        "alpha_output":if value.get("background").is_some(){"flatten"}else{"preserve"},
        "srgb_background":value.get("background").cloned(),
        "source_assignment":value["assignment"].as_str().map(assignment),
        "alpha_assignment":value.get("alpha_assignment").cloned()})
}
fn flags(settings: &Json) -> Vec<String> {
    let mut args = vec![
        "--float-curve".into(),
        settings["curve"].as_str().unwrap().into(),
        "--float-exposure-milli-ev".into(),
        settings["exposure_milli_ev"].to_string(),
        "--float-negative".into(),
        settings["negative"].as_str().unwrap().into(),
        "--float-alpha".into(),
        settings["alpha_output"].as_str().unwrap().into(),
    ];
    for (field, flag) in [
        ("source_assignment", "--float-source-space"),
        ("alpha_assignment", "--float-alpha-assignment"),
    ] {
        if let Some(value) = settings[field].as_str() {
            args.extend([flag.into(), value.into()]);
        }
    }
    if let Some(rgb) = settings["srgb_background"].as_array() {
        args.extend([
            "--float-background".into(),
            rgb.iter()
                .map(ToString::to_string)
                .collect::<Vec<_>>()
                .join(","),
        ]);
    }
    args
}
fn run(app: &Installation, args: &[String]) -> (bool, Json) {
    let result = Command::new(&app.cli)
        .args(args)
        .arg("--json")
        .output()
        .unwrap();
    let value = serde_json::from_slice(&result.stdout)
        .unwrap_or_else(|e| panic!("{e}:{}", String::from_utf8_lossy(&result.stderr)));
    (result.status.success(), value)
}
fn pixels(path: &Path, expected: &[u8]) {
    let document = paintcraft_formats::pcnd::read(&fs::read(path).unwrap()).unwrap();
    let layer = document.layers_in_order().next().unwrap();
    for (i, color) in expected.as_chunks::<4>().0.iter().enumerate() {
        assert_eq!(
            layer
                .raster
                .get(
                    (i % document.canvas.0 as usize) as i64,
                    (i / document.canvas.0 as usize) as i64
                )
                .0,
            *color
        );
    }
}
#[test]
#[ignore = "requires final PAINTCRAFT_V7_INSTALLATION or PAINTCRAFT_V7_HELPER"]
fn actual_cli_and_mcp_float_inspection_import_match_all_independent_cases() {
    let app = installation();
    let manifest: Json = serde_json::from_slice(
        &fs::read(formats().join("float-import-fixtures/manifest.json")).unwrap(),
    )
    .unwrap();
    let mut cli_pass = 0;
    let mut mcp_pass = 0;
    let mut refused = 0;
    for case in manifest["cases"].as_array().unwrap() {
        let original = fs::read(source(case)).unwrap();
        assert_eq!(
            hex(&Sha256::digest(&original)),
            case["source_sha256"].as_str().unwrap()
        );
        let input = app.fixture(case["file"].as_str().unwrap(), &original);
        let choice = settings(case);
        let policy = flags(&choice);
        let mut argv = vec!["inspect-float".into(), input.to_string_lossy().into_owned()];
        argv.extend(policy.clone());
        let (ok, inspection) = run(&app, &argv);
        let mut client = Client::at(&app.cli);
        client.ok("create_canvas", serde_json::json!({"width":2,"height":2}));
        client.ok("paint_stroke",serde_json::json!({"points":[{"x":0,"y":0},{"x":1,"y":0}],"color":"#FF0000FF","size":1,"brush":"round"}));
        let prior = client.ok("inspect_document", serde_json::json!({}));
        if !case["expected_error"].is_null() {
            assert!(!ok, "{}", case["id"]);
            assert_eq!(
                client.call(
                    "inspect_float",
                    serde_json::json!({"path":input,"settings":choice})
                )["isError"],
                true
            );
            assert_eq!(client.ok("inspect_document", serde_json::json!({})), prior);
            refused += 1;
            continue;
        }
        assert!(ok, "{}:{inspection}", case["id"]);
        assert_eq!(inspection["source_sha256"], case["source_sha256"]);
        assert_eq!(inspection["settings"], choice);
        assert_eq!(
            inspection["source_assessment"]["rgba_sha256"],
            case["rgba_sha256"]
        );
        let inspected = client.ok(
            "inspect_float",
            serde_json::json!({"path":input,"settings":choice}),
        );
        assert_eq!(
            inspected["assessment_sha256"],
            inspection["assessment_sha256"]
        );
        assert_eq!(client.ok("inspect_document", serde_json::json!({})), prior);
        let losses = inspection["source_assessment"]["losses"]
            .as_array()
            .unwrap()
            .iter()
            .map(|v| v.as_str().unwrap())
            .collect::<Vec<_>>()
            .join(",");
        let expected = fs::read(
            formats()
                .join("float-import-fixtures")
                .join(case["expected_rgba_file"].as_str().unwrap()),
        )
        .unwrap();
        assert_eq!(
            hex(&Sha256::digest(&expected)),
            case["rgba_sha256"].as_str().unwrap()
        );
        let output = app
            .root
            .join(format!("{}.pcnd", case["id"].as_str().unwrap()));
        let mut import = vec!["import-float".into(), input.to_string_lossy().into_owned()];
        import.extend(policy);
        import.extend([
            "--expect-input-sha256".into(),
            case["source_sha256"].as_str().unwrap().into(),
            "--expect-assessment-sha256".into(),
            inspection["assessment_sha256"].as_str().unwrap().into(),
            "--output".into(),
            output.to_string_lossy().into_owned(),
        ]);
        assert!(!run(&app, &import).0, "required loss consent");
        assert!(!output.exists());
        import.extend(["--allow-loss".into(), losses]);
        let (ok, receipt) = run(&app, &import);
        assert!(ok, "{receipt}");
        pixels(&output, &expected);
        cli_pass += 1;
        let mut request = serde_json::json!({"path":input,"settings":choice,"expect_input_sha256":case["source_sha256"],"expect_assessment_sha256":inspection["assessment_sha256"],"allow_losses":inspection["source_assessment"]["losses"]});
        assert_eq!(
            client.call("import_float", request.clone())["isError"],
            true,
            "dirty guard"
        );
        assert_eq!(client.ok("inspect_document", serde_json::json!({})), prior);
        request["discard_changes"] = serde_json::json!(true);
        request["expect_assessment_sha256"] = serde_json::json!("00".repeat(32));
        assert_eq!(
            client.call("import_float", request.clone())["isError"],
            true
        );
        assert_eq!(client.ok("inspect_document", serde_json::json!({})), prior);
        request["expect_assessment_sha256"] = inspection["assessment_sha256"].clone();
        let imported = client.ok("import_float", request);
        assert_eq!(imported["dirty"], true);
        let saved = app
            .root
            .join(format!("{}.mcp.pcnd", case["id"].as_str().unwrap()));
        client.ok("save_project", serde_json::json!({"path":saved}));
        pixels(&saved, &expected);
        mcp_pass += 1;
        assert_eq!(fs::read(&input).unwrap(), original);
    }
    assert_eq!((cli_pass, mcp_pass, refused), (12, 12, 5));
}

#[test]
#[ignore = "requires final PAINTCRAFT_V7_INSTALLATION or PAINTCRAFT_V7_HELPER"]
fn actual_float_source_guards_cancel_and_rollback_preserve_prior_workspace() {
    let app = installation();
    let manifest: Json = serde_json::from_slice(
        &fs::read(formats().join("float-import-fixtures/manifest.json")).unwrap(),
    )
    .unwrap();
    let case = manifest["cases"]
        .as_array()
        .unwrap()
        .iter()
        .find(|v| v["id"] == "hdr-clip")
        .unwrap();
    let original = fs::read(source(case)).unwrap();
    let input = app.fixture("original.hdr", &original);
    let choice = settings(case);
    let mut client = Client::at(&app.cli);
    client.ok("create_canvas", serde_json::json!({"width":3,"height":2}));
    client.ok("paint_stroke",serde_json::json!({"points":[{"x":0,"y":0},{"x":1,"y":0}],"color":"#FF0000FF","size":1,"brush":"round"}));
    let project = app.root.join("prior.pcnd");
    client.ok("save_project", serde_json::json!({"path":project}));
    let native_bytes = fs::read(&project).unwrap();
    let before = client.ok("inspect_document", serde_json::json!({}));
    eprintln!("float-guards phase=initial-inspection");
    let inspected = client.ok(
        "inspect_float",
        serde_json::json!({"path":input,"settings":choice}),
    );
    let request = serde_json::json!({"path":input,"settings":choice,"expect_input_sha256":case["source_sha256"],
        "expect_assessment_sha256":inspected["assessment_sha256"],"allow_losses":inspected["source_assessment"]["losses"],"discard_changes":true});
    for kind in [
        "raw-sha",
        "settings",
        "missing-loss",
        "unknown-loss",
        "changed-source",
    ] {
        eprintln!("float-guards phase=refusal kind={kind}");
        let mut bad = request.clone();
        match kind {
            "raw-sha" => bad["expect_input_sha256"] = serde_json::json!("00".repeat(32)),
            "settings" => bad["settings"]["exposure_milli_ev"] = serde_json::json!(1),
            "missing-loss" => bad["allow_losses"] = serde_json::json!([]),
            "unknown-loss" => bad["allow_losses"] = serde_json::json!(["invented"]),
            _ => {
                let mut changed = original.clone();
                let last = changed.len() - 1;
                changed[last] ^= 1;
                fs::write(&input, changed).unwrap();
            }
        }
        assert_eq!(client.call("import_float", bad)["isError"], true, "{kind}");
        assert_eq!(
            client.ok("inspect_document", serde_json::json!({})),
            before,
            "{kind}"
        );
        assert_eq!(fs::read(&project).unwrap(), native_bytes);
        fs::write(&input, &original).unwrap();
    }
    eprintln!("float-guards phase=saved-binding-control");
    // Binding preservation has a positive control and a hostile external rewrite.
    client.ok(
        "save_project",
        serde_json::json!({"path":project,"overwrite":true}),
    );
    fs::write(&project, b"external replacement bytes").unwrap();
    assert_eq!(
        client.call(
            "save_project",
            serde_json::json!({"path":project,"overwrite":true})
        )["isError"],
        true
    );
    assert_eq!(fs::read(&project).unwrap(), b"external replacement bytes");
    fs::write(&project, &native_bytes).unwrap();

    eprintln!("float-guards phase=rpc-cancellation");
    client.cancel_call("import_float", request.clone());
    assert_eq!(client.ok("inspect_document", serde_json::json!({})), before);
    assert_eq!(fs::read(&project).unwrap(), native_bytes);
    assert_eq!(fs::read(&input).unwrap(), original);

    eprintln!("float-guards phase=cli-source-alias");
    let alias = app.root.join("source-alias.pcnd");
    fs::hard_link(&input, &alias).unwrap();
    let mut argv = vec!["import-float".into(), input.to_string_lossy().into_owned()];
    argv.extend(flags(&choice));
    argv.extend([
        "--expect-input-sha256".into(),
        case["source_sha256"].as_str().unwrap().into(),
        "--expect-assessment-sha256".into(),
        inspected["assessment_sha256"].as_str().unwrap().into(),
        "--allow-loss".into(),
        inspected["source_assessment"]["losses"]
            .as_array()
            .unwrap()
            .iter()
            .map(|v| v.as_str().unwrap())
            .collect::<Vec<_>>()
            .join(","),
        "--output".into(),
        alias.to_string_lossy().into_owned(),
        "--overwrite".into(),
    ]);
    let entries = || {
        fs::read_dir(&app.root)
            .unwrap()
            .map(|e| e.unwrap().file_name())
            .collect::<std::collections::BTreeSet<_>>()
    };
    let before_entries = entries();
    assert!(!run(&app, &argv).0);
    assert_eq!(entries(), before_entries);
    assert_eq!(fs::read(&input).unwrap(), original);
    assert_eq!(fs::read(&alias).unwrap(), original);

    eprintln!("float-guards phase=positive-adoption");
    client.ok("import_float", request);
    let imported = client.ok("inspect_document", serde_json::json!({}));
    assert_eq!(imported["dirty"], true);
    eprintln!("float-guards phase=renamed-source-save-controls");
    let renamed = app.root.join("renamed.hdr");
    fs::rename(&input, &renamed).unwrap();
    let unrelated = app.root.join("unrelated.pcnd");
    client.ok("save_project", serde_json::json!({"path":unrelated}));
    let expected = fs::read(formats().join("float-import-fixtures/hdr-clip.rgba")).unwrap();
    pixels(&unrelated, &expected);
    let saved = client.ok("inspect_document", serde_json::json!({}));
    let before_entries = entries();
    assert_eq!(
        client.call(
            "save_project",
            serde_json::json!({"path":alias,"overwrite":true})
        )["isError"],
        true
    );
    assert_eq!(entries(), before_entries);
    assert_eq!(client.ok("inspect_document", serde_json::json!({})), saved);
    assert_eq!(fs::read(&renamed).unwrap(), original);
    let after = app.root.join("after-alias.pcnd");
    client.ok("save_project", serde_json::json!({"path":after}));
    pixels(&after, &expected);
}
