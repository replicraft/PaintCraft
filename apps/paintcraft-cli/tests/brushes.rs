use serde_json::Value;
use std::{
    fs,
    process::{Command, Output},
};
const SOURCE: &[u8] = include_bytes!(
    "../../../crates/services/paintcraft-formats/tests/abr-fixtures/native-presets.abr"
);
fn run(args: &[&str]) -> Output {
    Command::new(env!("CARGO_BIN_EXE_paintcraft-cli"))
        .args(args)
        .output()
        .unwrap()
}
fn ok(output: Output) -> Value {
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    serde_json::from_slice(&output.stdout).unwrap()
}
#[test]
fn brush_assets_inspect_and_paint_exact_native_pixels_without_source_mutation() {
    let root = std::env::temp_dir().join(format!("paintcraft-cli-abr-{}", std::process::id()));
    fs::create_dir_all(&root).unwrap();
    let input = root.join("source.abr");
    let project = root.join("base.pcnd");
    let output = root.join("painted.pcnd");
    let points = root.join("points.json");
    fs::write(&input, SOURCE).unwrap();
    fs::write(&points, b"[[3,3]]").unwrap();
    let input = input.to_str().unwrap();
    let project = project.to_str().unwrap();
    let output = output.to_str().unwrap();
    let points = points.to_str().unwrap();
    let schema = ok(run(&["schema", "brush-inspection"]));
    assert_eq!(schema["properties"]["presets"]["maxItems"], 256);
    let inspection = ok(run(&["brush", "inspect", input, "--json"]));
    assert_eq!(inspection["presets"][0]["qualified"], true);
    assert_eq!(inspection["presets"][2]["qualified"], false);
    assert!(
        !run(&["brush", "inspect", input, "--frontend", "classic-v11"])
            .status
            .success()
    );
    ok(run(&[
        "new", "--width", "8", "--height", "8", "-o", project, "--json",
    ]));
    let args = [
        "brush",
        "paint",
        project,
        "--brush",
        input,
        "--preset",
        "0",
        "--policy",
        "native-grid-v1",
        "--points",
        points,
        "--color",
        "#123456FF",
        "-o",
        output,
        "--json",
    ];
    let empty_output = root.join("empty-tip.pcnd");
    let mut empty = args.to_vec();
    empty[6] = "14";
    empty[14] = empty_output.to_str().unwrap();
    assert_eq!(ok(run(&empty))["engine"]["committed"], false);
    assert!(
        paintcraft_formats::pcnd::read(&fs::read(&empty_output).unwrap())
            .unwrap()
            .roots
            .is_empty()
    );
    let result = ok(run(&args));
    assert_eq!(result["engine"]["committed"], true);
    let doc = paintcraft_formats::pcnd::read(&fs::read(output).unwrap()).unwrap();
    let layer = &doc.layers[&doc.roots[0]];
    for (x, y, alpha) in [
        (2, 2, 0),
        (3, 2, 1),
        (4, 2, 127),
        (2, 3, 128),
        (3, 3, 254),
        (4, 3, 255),
    ] {
        assert_eq!(layer.raster.get(x, y).a(), alpha);
    }
    let before = fs::read(output).unwrap();
    let mut bad = args.to_vec();
    bad[6] = "2";
    bad.push("--overwrite");
    let rejected = run(&bad);
    assert_eq!(rejected.status.code(), Some(3));
    let error: Value = serde_json::from_slice(&rejected.stdout).unwrap();
    assert_eq!(error["engine"]["code"], "capability.unsupported");
    assert_eq!(fs::read(output).unwrap(), before);
    assert_eq!(fs::read(input).unwrap(), SOURCE);
    let alias = root.join("source-alias.pcnd");
    fs::hard_link(input, &alias).unwrap();
    let mut alias_args = args.to_vec();
    alias_args[14] = alias.to_str().unwrap();
    alias_args.push("--overwrite");
    assert!(!run(&alias_args).status.success());
    assert_eq!(fs::read(input).unwrap(), SOURCE);
    fs::remove_dir_all(root).unwrap();
}
