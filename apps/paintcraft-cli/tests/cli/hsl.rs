//! Real catalog Hue/Saturation packets and copies, independent literal pixel oracle.
use super::catalog::read_catalog;
use super::*;
use paintcraft_core::types::Rgba;
use paintcraft_document::{POLICY, new_document};
use paintcraft_formats::pcnd;
use serde_json::{Value as Json, json};

fn descriptor(parameters: [i16; 3]) -> Json {
    json!({"kind":"hue-saturation-rgb-v1","hue_degrees":parameters[0],"saturation_hundredths":parameters[1],"lightness_hundredths":parameters[2]})
}
fn packet(state: &str, requirements: Json, operations: Json) -> Json {
    json!({"schema":"paintcraft.batch/catalog-v1","atomic":true,"policy":POLICY,
        "base":{"state_id":state,"publication_epoch":"0"},
        "required_filters":requirements,"operations":operations})
}
fn apply(input: &str, ops: &str, output: &str, value: &Json) -> Run {
    fs::write(ops, serde_json::to_vec(value).unwrap()).unwrap();
    run(&["apply", input, "--operations", ops, "-o", output, "--json"])
}
fn ok(result: &Run) -> Json {
    assert_eq!(result.code, 0, "{} {}", result.stdout, result.stderr);
    serde_json::from_str(&result.stdout).unwrap()
}
fn record(name: &str, value: &Json) {
    if let Some(root) = std::env::var_os("PAINTCRAFT_HSL_RECEIPTS") {
        let root = PathBuf::from(root);
        fs::create_dir_all(&root).unwrap();
        fs::write(
            root.join(format!("{name}.json")),
            serde_json::to_vec_pretty(value).unwrap(),
        )
        .unwrap();
    }
}
#[test]
fn hsl_cli_disabled_signed_intent_noop_save_inspect_and_last_catalog_downgrade() {
    let dir = Scratch::new("hsl-intent");
    for (amount, parameters) in [(-180, [-180, 0, 0]), (0, [0, 0, 0]), (180, [180, 0, 0])] {
        let input = dir.path(&format!("source-{amount}.pcnd"));
        let output = dir.path(&format!("hsl-{amount}.pcnd"));
        let noop = dir.path(&format!("noop-{amount}.pcnd"));
        let removed = dir.path(&format!("removed-{amount}.pcnd"));
        let ops = dir.path("ops.json");
        let doc = new_document(2, 1, Some(Rgba([64, 16, 128, 0])), 0, POLICY).unwrap();
        let layer = doc.roots[0].to_string();
        let original = pcnd::write_compact(&doc).unwrap();
        fs::write(&input, &original).unwrap();
        let mut operations = vec![
            json!({"op":"filter.add","layer_ref":{"id":layer},"ref":"hsl","enabled":false,"descriptor":descriptor(parameters)}),
        ];
        let mut requirements = vec!["hue-saturation-rgb-v1"];
        if amount == 0 {
            operations.push(json!({"op":"filter.add","layer_ref":{"id":layer},"ref":"old","enabled":false,"descriptor":{"kind":"invert-rgb-v1"}}));
            requirements.push("invert-rgb-v1");
        }
        requirements.sort_unstable();
        let added = ok(&apply(
            &input,
            &ops,
            &output,
            &packet(
                &doc.state_id.to_string(),
                json!(requirements),
                json!(operations),
            ),
        ));
        assert_eq!(added["schema"], "paintcraft.file-command/catalog-v1");
        assert_eq!(added["engine"]["schema"], "paintcraft.result/catalog-v1");
        assert_eq!(added["engine"]["required_filters"], json!(requirements));
        record(&format!("cli-{amount}-envelope"), &added);
        record(&format!("cli-{amount}-result"), &added["engine"]);
        let saved = fs::read(&output).unwrap();
        assert_eq!(saved[4], 6);
        let reopened = read_catalog(&saved);
        assert_eq!(reopened.document_id, doc.document_id);
        assert_eq!(
            reopened.layers[&doc.roots[0]].raster,
            doc.layers[&doc.roots[0]].raster
        );
        let filter = &reopened.layers[&doc.roots[0]].filters.as_slice()[0];
        assert!(!filter.enabled());
        let (semantic, used) = filter.descriptor().semantic_parameter_bytes();
        assert_eq!(used, 6);
        let expected: Vec<u8> = parameters.into_iter().flat_map(i16::to_be_bytes).collect();
        assert_eq!(&semantic[..used], expected);
        assert_eq!(
            filter.descriptor().to_value(),
            json::parse(&serde_json::to_vec(&descriptor(parameters)).unwrap()).unwrap()
        );
        let inspected = ok(&run(&["inspect", &output, "--json"]));
        assert_eq!(inspected["schema"], "paintcraft.inspect/catalog-v1");
        assert_eq!(inspected["required_filters"], json!(requirements));
        assert_eq!(
            inspected["assessment"]["required_filters"],
            json!(
                requirements
                    .iter()
                    .map(|id| json!({"profile_id":id,"codec_id":"params-v1"}))
                    .collect::<Vec<_>>()
            )
        );
        record(&format!("cli-{amount}-inspect"), &inspected);
        let unchanged = ok(&apply(
            &output,
            &ops,
            &noop,
            &packet(
                &reopened.state_id.to_string(),
                json!(["hue-saturation-rgb-v1"]),
                json!([
            {"op":"filter.update","layer_ref":{"id":layer},"filter_ref":{"id":filter.id().to_string()},"descriptor":descriptor(parameters)}]),
            ),
        ));
        assert_eq!(unchanged["engine"]["committed"], false);
        assert_eq!(unchanged["engine"]["required_filters"], json!(requirements));
        assert_eq!(fs::read(&noop).unwrap(), saved);
        record(&format!("cli-{amount}-noop-result"), &unchanged["engine"]);
        let removed_receipt = ok(&apply(
            &output,
            &ops,
            &removed,
            &packet(
                &reopened.state_id.to_string(),
                json!([]),
                json!([
            {"op":"filter.remove","layer_ref":{"id":layer},"filter_ref":{"id":filter.id().to_string()}}]),
            ),
        ));
        assert_eq!(removed_receipt["engine"]["schema"], "paintcraft.result/v1");
        assert_eq!(removed_receipt["schema"], "paintcraft.file-command/v1");
        assert!(removed_receipt["engine"].get("required_filters").is_none());
        assert_eq!(
            fs::read(&removed).unwrap()[4],
            if amount == 0 { 3 } else { 1 }
        );
        assert_eq!(fs::read(&input).unwrap(), original);
        assert_eq!(fs::read(&output).unwrap(), saved);
    }
}
#[test]
fn hsl_cli_active_png_bake_consent_literals_and_legacy_packet_refusals() {
    for (case, parameters, expected_rgb) in [
        ("all", [137, 73, -19], [47, 34, 0]),
        ("negative-half-turn", [-180, 0, 0], [80, 128, 16]),
        ("positive-half-turn", [180, 0, 0], [80, 128, 16]),
    ] {
        let dir = Scratch::new("hsl-render");
        let input = dir.path("source.pcnd");
        let output = dir.path("hsl.pcnd");
        let ops = dir.path("ops.json");
        let png = dir.path("render.png");
        let doc = new_document(2, 1, Some(Rgba([64, 16, 128, 255])), 0, POLICY).unwrap();
        let source = pcnd::write_compact(&doc).unwrap();
        fs::write(&input, &source).unwrap();
        let layer = doc.roots[0].to_string();
        let batch = packet(
            &doc.state_id.to_string(),
            json!(["hue-saturation-rgb-v1"]),
            json!([
        {"op":"filter.add","layer_ref":{"id":layer},"ref":"hsl","descriptor":descriptor(parameters)}]),
        );
        let added = ok(&apply(&input, &ops, &output, &batch));
        record(&format!("cli-active-{case}-envelope"), &added);
        record(&format!("cli-active-{case}-result"), &added["engine"]);
        let saved = fs::read(&output).unwrap();
        assert_eq!(saved[4], 6);
        let decoded_source = read_catalog(&saved);
        let stored = decoded_source.layers[&doc.roots[0]].filters.as_slice()[0].descriptor();
        let (bytes, length) = stored.semantic_parameter_bytes();
        assert_eq!(length, 6);
        let expected: Vec<u8> = parameters.into_iter().flat_map(i16::to_be_bytes).collect();
        assert_eq!(&bytes[..length], expected);

        let before = ok(&run(&["inspect", &output, "--json"]));
        record(&format!("cli-active-{case}-inspect"), &before);
        let refused = run(&["render", &output, "-o", &png, "--json"]);
        assert_ne!(refused.code, 0);
        assert!(refused.stdout.contains("explicit consent to bake"));
        assert!(!Path::new(&png).exists());
        ok(&run(&[
            "render",
            &output,
            "-o",
            &png,
            "--allow-loss",
            "filters-baked",
            "--json",
        ]));
        assert_eq!(
            decode_png(&png),
            (
                2,
                1,
                [expected_rgb[0], expected_rgb[1], expected_rgb[2], 255].repeat(2)
            )
        );
        assert_eq!(ok(&run(&["inspect", &output, "--json"])), before);
        assert_eq!(
            read_catalog(&saved).layers[&doc.roots[0]].raster,
            doc.layers[&doc.roots[0]].raster
        );
        assert_eq!(fs::read(&output).unwrap(), saved);
        assert_eq!(fs::read(&input).unwrap(), source);
        for version in 1..=5 {
            let mut old = batch.clone();
            old["schema"] = json!(format!("paintcraft.batch/v{version}"));
            old.as_object_mut().unwrap().remove("required_filters");
            let dest = dir.path(&format!("refuse-{version}.pcnd"));
            let refused = apply(&input, &ops, &dest, &old);
            assert_ne!(refused.code, 0);
            assert!(
                refused.stdout.contains("input.schema"),
                "{}",
                refused.stdout
            );
            assert!(!Path::new(&dest).exists());
            assert_eq!(fs::read(&input).unwrap(), source);
        }
    }
}
