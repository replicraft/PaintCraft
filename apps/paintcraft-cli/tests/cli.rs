//! End-to-end tests of the real binary, including the M1 definition-of-done
//! scenario: create, edit across tile boundaries, inspect, render PNG.

use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

use paintcraft_types::json::{self, Value};

struct Run {
    code: i32,
    stdout: String,
    stderr: String,
}

impl Run {
    fn json(&self) -> Value {
        json::parse(self.stdout.as_bytes())
            .unwrap_or_else(|e| panic!("stdout is not one JSON object: {e}\n{}", self.stdout))
    }
}

fn run(args: &[&str]) -> Run {
    let out = Command::new(env!("CARGO_BIN_EXE_paintcraft-cli"))
        .args(args)
        .output()
        .unwrap();
    Run {
        code: out.status.code().unwrap(),
        stdout: String::from_utf8(out.stdout).unwrap(),
        stderr: String::from_utf8(out.stderr).unwrap(),
    }
}

struct Scratch(PathBuf);

impl Scratch {
    fn new(name: &str) -> Scratch {
        let dir =
            std::env::temp_dir().join(format!("paintcraft-cli-{name}-{}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).unwrap();
        Scratch(dir)
    }

    fn path(&self, name: &str) -> String {
        self.0.join(name).to_string_lossy().into_owned()
    }
}

impl Drop for Scratch {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

fn text<'a>(v: &'a Value, path: &[&str]) -> &'a str {
    path.iter()
        .fold(v, |v, k| v.get(k).unwrap_or_else(|| panic!("missing {k}")))
        .as_str()
        .unwrap()
}

fn vector(name: &str) -> Value {
    json::parse(
        &fs::read(
            Path::new(env!("CARGO_MANIFEST_DIR"))
                .join("../../conformance")
                .join(name),
        )
        .unwrap(),
    )
    .unwrap()
}

fn m1_batch() -> String {
    let steps = vector("M1-001.json");
    let batch = steps.get("steps").unwrap().as_array().unwrap()[0]
        .get("batch")
        .unwrap()
        .clone();
    String::from_utf8(json::canonical(&batch)).unwrap()
}

fn decode_png(path: &str) -> (u32, u32, Vec<u8>) {
    let mut r = png::Decoder::new(std::io::Cursor::new(fs::read(path).unwrap()))
        .read_info()
        .unwrap();
    let mut buf = vec![0; r.output_buffer_size().unwrap()];
    let info = r.next_frame(&mut buf).unwrap();
    assert_eq!(info.color_type, png::ColorType::Rgba);
    (info.width, info.height, buf[..info.buffer_size()].to_vec())
}

#[test]
fn m1_scenario_new_apply_inspect_render() {
    let s = Scratch::new("m1");
    let (base, edited, png_path, ops) = (
        s.path("base.pcnd"),
        s.path("edited.pcnd"),
        s.path("out.png"),
        s.path("ops.json"),
    );

    let r = run(&[
        "new", "--width", "320", "--height", "320", "-o", &base, "--json",
    ]);
    assert_eq!(r.code, 0, "{}", r.stderr);
    let env = r.json();
    assert_eq!(text(&env, &["outcome"]), "success");
    assert_eq!(env.get("engine"), Some(&Value::Null));
    assert_eq!(text(&env, &["file", "replacement"]), "replaced");
    assert_eq!(text(&env, &["file", "durability"]), "confirmed");
    assert_eq!(
        env.get("file").unwrap().get("source_sha256"),
        Some(&Value::Null)
    );

    // The vector's base ticket is exactly what `new` creates deterministically.
    fs::write(&ops, m1_batch()).unwrap();
    let before = fs::read(&base).unwrap();
    let ins = run(&["inspect", &base, "--json"]).json();
    assert_eq!(text(&ins, &["ticket", "publication_epoch"]), "0");
    assert_eq!(text(&ins, &["assessment", "mode"]), "editable");
    assert_eq!(
        ins.get("document")
            .unwrap()
            .get("layers")
            .unwrap()
            .as_array()
            .unwrap()
            .len(),
        0
    );

    let sha = text(&ins, &["input_sha256"]).to_owned();
    let r = run(&[
        "apply",
        &base,
        "--operations",
        &ops,
        "-o",
        &edited,
        "--expect-input-sha256",
        &sha,
        "--json",
    ]);
    assert_eq!(r.code, 0, "{} {}", r.stderr, r.stdout);
    let env = r.json();
    assert_eq!(
        env.get("engine").unwrap().get("committed"),
        Some(&Value::Bool(true))
    );
    assert_eq!(
        text(&env, &["engine", "state_id"]),
        text(&env, &["file", "source_state_id"])
    );
    assert_eq!(text(&env, &["file", "source_sha256"]), sha);
    assert_eq!(
        fs::read(&base).unwrap(),
        before,
        "the input is never modified"
    );

    // The vector deliberately stores hidden RGB under alpha zero; a flat PNG cannot keep it.
    let refused = run(&["render", &edited, "-o", &png_path, "--json"]);
    assert_eq!(refused.code, 3);
    assert!(!Path::new(&png_path).exists());
    let r = run(&[
        "render",
        &edited,
        "-o",
        &png_path,
        "--allow-loss",
        "hidden-rgb",
        "--json",
    ]);
    assert_eq!(r.code, 0, "{}", r.stderr);
    let env = r.json();
    assert_eq!(
        env.get("file").unwrap().get("losses_acknowledged"),
        Some(&Value::Array(vec![Value::str("hidden-rgb")]))
    );
    assert_eq!(text(&env, &["file", "operation"]), "export");
    assert_eq!(
        env.get("file").unwrap().get("acknowledged_state_id"),
        Some(&Value::Null),
        "an export never clears native dirty state"
    );
    assert_eq!(text(&env, &["file", "retry"]), "no-automatic-retry");

    // The PNG carries the conformance vector's exact pixels across the tile edges.
    let (w, h, px) = decode_png(&png_path);
    assert_eq!((w, h), (320, 320));
    let spots = vector("M1-001.json");
    let check = spots.get("steps").unwrap().as_array().unwrap()[1]
        .get("expect")
        .unwrap()
        .get("render_pixels")
        .unwrap()
        .as_array()
        .unwrap();
    assert!(check.len() >= 17);
    for spot in check {
        let (x, y) = (
            spot.get("x").unwrap().as_i64().unwrap() as usize,
            spot.get("y").unwrap().as_i64().unwrap() as usize,
        );
        let got: String = px[(y * 320 + x) * 4..][..4]
            .iter()
            .map(|b| format!("{b:02x}"))
            .collect();
        assert_eq!(got, text(spot, &["rgba"]), "pixel {x},{y}");
    }

    // Re-running against the committed output is stale: the new base ticket no longer matches.
    let again = run(&[
        "apply",
        &edited,
        "--operations",
        &ops,
        "-o",
        &s.path("again.pcnd"),
        "--json",
    ]);
    assert_eq!(again.code, 4);
    assert_eq!(
        text(&again.json(), &["engine", "code"]),
        "conflict.stale_base"
    );
    assert!(!Path::new(&s.path("again.pcnd")).exists());
}

#[test]
fn new_is_deterministic_and_refuses_to_clobber() {
    let s = Scratch::new("new");
    let (a, b) = (s.path("a.pcnd"), s.path("b.pcnd"));
    for p in [&a, &b] {
        assert_eq!(
            run(&[
                "new",
                "--width",
                "7",
                "--height",
                "5",
                "--background",
                "#FFFFFFFF",
                "-o",
                p
            ])
            .code,
            0
        );
    }
    assert_eq!(fs::read(&a).unwrap(), fs::read(&b).unwrap());
    let r = run(&["new", "--width", "7", "--height", "5", "-o", &a, "--json"]);
    assert_eq!(r.code, 2);
    let env = r.json();
    assert_eq!(
        (text(&env, &["outcome"]), text(&env, &["engine", "code"])),
        ("engine-rejected", "target.invalid")
    );
    assert_eq!(env.get("file"), Some(&Value::Null));
    assert_eq!(
        fs::read(&a).unwrap(),
        fs::read(&b).unwrap(),
        "the existing output is intact"
    );
    assert_eq!(
        run(&[
            "new",
            "--width",
            "7",
            "--height",
            "5",
            "-o",
            &a,
            "--overwrite"
        ])
        .code,
        0
    );
    assert_eq!(
        run(&[
            "new",
            "--width",
            "0",
            "--height",
            "5",
            "-o",
            &s.path("z.pcnd")
        ])
        .code,
        2
    );
    assert_eq!(
        run(&[
            "new",
            "--width",
            "5",
            "--height",
            "5",
            "--policy",
            "paintcraft.core/missing-v1",
            "-o",
            &s.path("z.pcnd")
        ])
        .code,
        3
    );
}

#[test]
fn input_hash_guard_runs_before_parsing() {
    let s = Scratch::new("guard");
    let (base, ops, out) = (s.path("base.pcnd"), s.path("ops.json"), s.path("out.pcnd"));
    run(&["new", "--width", "320", "--height", "320", "-o", &base]);
    fs::write(&ops, m1_batch()).unwrap();
    let wrong = "0".repeat(64);
    let r = run(&[
        "apply",
        &base,
        "--operations",
        &ops,
        "-o",
        &out,
        "--expect-input-sha256",
        &wrong,
        "--json",
    ]);
    assert_eq!(r.code, 4);
    assert_eq!(text(&r.json(), &["engine", "code"]), "conflict.stale_base");
    assert!(!Path::new(&out).exists());
    // The guard is checked even when the input is not a valid document.
    let junk = s.path("junk.pcnd");
    fs::write(&junk, b"not a zip").unwrap();
    assert_eq!(
        run(&[
            "apply",
            &junk,
            "--operations",
            &ops,
            "-o",
            &out,
            "--expect-input-sha256",
            &wrong
        ])
        .code,
        4
    );
    assert_eq!(
        run(&["apply", &junk, "--operations", &ops, "-o", &out]).code,
        6
    );
    assert_eq!(
        run(&[
            "apply",
            &base,
            "--operations",
            &ops,
            "-o",
            &out,
            "--expect-input-sha256",
            "ABC"
        ])
        .code,
        2
    );
}

#[test]
fn noop_apply_still_writes_and_reports_uncommitted() {
    let s = Scratch::new("noop");
    let (base, ops, out) = (s.path("base.pcnd"), s.path("ops.json"), s.path("out.pcnd"));
    run(&["new", "--width", "4", "--height", "4", "-o", &base]);
    let state = text(
        &run(&["inspect", &base, "--json"]).json(),
        &["ticket", "state_id"],
    )
    .to_owned();
    fs::write(&ops, format!(r#"{{"schema":"paintcraft.batch/v1","atomic":true,"policy":"paintcraft.core/default-v1","base":{{"state_id":"{state}","publication_epoch":"0"}},"operations":[{{"op":"selection.clear"}}]}}"#)).unwrap();
    let r = run(&["apply", &base, "--operations", &ops, "-o", &out, "--json"]);
    assert_eq!(r.code, 0, "{}", r.stderr);
    let env = r.json();
    assert_eq!(
        env.get("engine").unwrap().get("committed"),
        Some(&Value::Bool(false))
    );
    assert_eq!(text(&env, &["engine", "state_id"]), state);
    assert_eq!(
        fs::read(&out).unwrap(),
        fs::read(&base).unwrap(),
        "canonical rewrite of an unchanged state"
    );
}

#[test]
fn batch_errors_report_the_failing_operation_and_write_nothing() {
    let s = Scratch::new("err");
    let (base, ops, out) = (s.path("base.pcnd"), s.path("ops.json"), s.path("out.pcnd"));
    run(&["new", "--width", "4", "--height", "4", "-o", &base]);
    let state = text(
        &run(&["inspect", &base, "--json"]).json(),
        &["ticket", "state_id"],
    )
    .to_owned();
    let batch = |ops: &str| {
        format!(
            r#"{{"schema":"paintcraft.batch/v1","atomic":true,"policy":"paintcraft.core/default-v1","base":{{"state_id":"{state}","publication_epoch":"0"}},"operations":{ops}}}"#
        )
    };
    fs::write(
        &ops,
        batch(r#"[{"op":"layer.delete","layer_ref":{"ref":"nope"}}]"#),
    )
    .unwrap();
    let r = run(&["apply", &base, "--operations", &ops, "-o", &out, "--json"]);
    assert_eq!(r.code, 2);
    let env = r.json();
    assert_eq!(text(&env, &["engine", "code"]), "reference.unknown_alias");
    assert_eq!(
        env.get("engine").unwrap().get("operation_index"),
        Some(&Value::Int(0))
    );
    fs::write(&ops, b"{\"a\":1,\"a\":2}").unwrap();
    assert_eq!(
        run(&["apply", &base, "--operations", &ops, "-o", &out]).code,
        2,
        "duplicate keys are a schema error"
    );
    assert!(!Path::new(&out).exists());
}

#[test]
fn render_flatten_needs_explicit_acknowledgement() {
    let s = Scratch::new("render");
    let (base, ops, out, png_path) = (
        s.path("base.pcnd"),
        s.path("ops.json"),
        s.path("out.pcnd"),
        s.path("o.png"),
    );
    run(&[
        "new",
        "--width",
        "2",
        "--height",
        "2",
        "--background",
        "#FFFFFFFF",
        "-o",
        &base,
    ]);
    let state = text(
        &run(&["inspect", &base, "--json"]).json(),
        &["ticket", "state_id"],
    )
    .to_owned();
    fs::write(&ops, format!(r##"{{"schema":"paintcraft.batch/v1","atomic":true,"policy":"paintcraft.core/default-v1","base":{{"state_id":"{state}","publication_epoch":"0"}},"operations":[{{"op":"layer.add","ref":"a","name":"A","width":2,"height":2,"origin":{{"x":0,"y":0}},"default_pixel":"#FF000080"}}]}}"##)).unwrap();
    assert_eq!(
        run(&["apply", &base, "--operations", &ops, "-o", &out]).code,
        0
    );
    let r = run(&["render", &out, "-o", &png_path, "--json"]);
    assert_eq!(r.code, 3);
    assert!(!Path::new(&png_path).exists());
    assert_eq!(
        run(&["render", &out, "-o", &png_path, "--on-loss", "warn"]).code,
        2
    );
    let r = run(&[
        "render",
        &out,
        "-o",
        &png_path,
        "--allow-loss",
        "flatten",
        "--json",
    ]);
    assert_eq!(r.code, 0, "{}", r.stderr);
    assert_eq!(
        r.json().get("file").unwrap().get("losses_acknowledged"),
        Some(&Value::Array(vec![Value::str("flatten")]))
    );
    assert_eq!(&decode_png(&png_path).2[..4], &[255, 127, 127, 255]);
}

#[test]
fn inspect_rejects_unsupported_versions_and_corruption_with_stable_codes() {
    let s = Scratch::new("inspect");
    let spec =
        fs::read(Path::new(env!("CARGO_MANIFEST_DIR")).join("../../conformance/minimal.pcnd"))
            .unwrap();
    let ok = s.path("ok.pcnd");
    fs::write(&ok, &spec).unwrap();
    let r = run(&["inspect", &ok, "--json"]);
    assert_eq!(r.code, 0);
    assert_eq!(
        r.json()
            .get("document")
            .unwrap()
            .get("canvas")
            .unwrap()
            .get("width"),
        Some(&Value::Int(2))
    );

    // Same bytes with a future feature in the manifest: rejected, never editable.
    let mut future = spec.clone();
    let needle = b"layers.flat";
    let at = future
        .windows(needle.len())
        .position(|w| w == needle)
        .unwrap();
    future[at..at + needle.len()].copy_from_slice(b"layers.curv");
    // The CRC no longer matches, so this is corruption (6); a consistent edit is covered in the unit tests.
    let bad = s.path("bad.pcnd");
    fs::write(&bad, &future).unwrap();
    assert_eq!(run(&["inspect", &bad, "--json"]).code, 6);
    assert_eq!(run(&["inspect", &s.path("missing.pcnd")]).code, 6);
    assert_eq!(run(&["inspect"]).code, 2);
}

#[test]
fn schema_capabilities_and_usage() {
    let schema = run(&["schema", "batch", "--json"]);
    assert_eq!(schema.code, 0);
    assert_eq!(
        text(&schema.json(), &["$id"]),
        "urn:paintcraft:schema:batch-v1"
    );
    let v2 = run(&["schema", "batch-v2"]);
    assert_eq!(text(&v2.json(), &["$id"]), "urn:paintcraft:schema:batch-v2");
    assert_eq!(run(&["schema", "result"]).code, 2);
    let caps = run(&["capabilities", "--json"]).json();
    let rows = caps.get("capabilities").unwrap().as_array().unwrap();
    assert!(rows.len() >= 5);
    for row in rows {
        assert_eq!(
            row.get("release_qualified"),
            Some(&Value::Bool(false)),
            "nothing is release-qualified"
        );
    }
    assert_eq!(run(&[]).code, 2);
    assert_eq!(run(&["frobnicate"]).code, 2);
    assert_eq!(run(&["--version"]).code, 0);
    assert!(
        run(&["new", "--width", "1", "--height", "1", "--bogus"])
            .stderr
            .contains("unexpected argument")
    );
}

#[test]
fn an_unwritable_destination_is_a_known_non_replacement() {
    let s = Scratch::new("unwritable");
    let dest = s.0.join("no-such-dir").join("a.pcnd");
    let r = run(&[
        "new",
        "--width",
        "2",
        "--height",
        "2",
        "-o",
        &dest.to_string_lossy(),
        "--json",
    ]);
    assert_eq!(r.code, 6);
    let env = r.json();
    assert_eq!(text(&env, &["outcome"]), "file-incomplete");
    assert_eq!(text(&env, &["file", "replacement"]), "not-replaced");
    assert_eq!(text(&env, &["file", "retry"]), "allowed-new-request");
    assert_eq!(
        env.get("file").unwrap().get("acknowledged_state_id"),
        Some(&Value::Null)
    );
}

#[test]
fn import_and_export_cover_png_bmp_and_jpeg_with_explicit_losses() {
    let s = Scratch::new("images");
    let (doc, out_png, out_bmp, out_jpg, back) = (
        s.path("d.pcnd"),
        s.path("o.png"),
        s.path("o.bmp"),
        s.path("o.jpg"),
        s.path("back.pcnd"),
    );
    run(&[
        "new",
        "--width",
        "8",
        "--height",
        "4",
        "--background",
        "#336699FF",
        "-o",
        &doc,
    ]);
    assert_eq!(run(&["render", &doc, "-o", &out_png]).code, 0);
    assert_eq!(run(&["render", &doc, "-o", &out_bmp]).code, 0);

    // PNG and BMP come back as the same picture; importing twice gives byte-identical documents.
    let r = run(&["import", &out_png, "-o", &back, "--json"]);
    assert_eq!(r.code, 0, "{}", r.stderr);
    let first = fs::read(&back).unwrap();
    let ins = run(&["inspect", &back, "--json"]).json();
    assert_eq!(
        ins.get("document")
            .unwrap()
            .get("canvas")
            .unwrap()
            .get("width"),
        Some(&Value::Int(8))
    );
    let again = s.path("again.pcnd");
    assert_eq!(run(&["import", &out_png, "-o", &again]).code, 0);
    assert_eq!(
        fs::read(&again).unwrap(),
        first,
        "the same file imports to the same bytes"
    );
    assert_eq!(
        run(&["import", &out_bmp, "-o", &back, "--overwrite"]).code,
        0
    );
    let (w, h, px) = decode_png(&out_png);
    assert_eq!((w, h, &px[..4]), (8, 4, &[0x33, 0x66, 0x99, 255][..]));

    // JPEG needs the lossy acknowledgement; the file is only written once it is given.
    let r = run(&["render", &doc, "-o", &out_jpg, "--json"]);
    assert_eq!(r.code, 3);
    assert!(text(&r.json(), &["engine", "message"]).contains("lossy-jpeg"));
    assert!(!Path::new(&out_jpg).exists());
    let r = run(&[
        "render",
        &doc,
        "-o",
        &out_jpg,
        "--allow-loss",
        "lossy-jpeg",
        "--quality",
        "95",
        "--json",
    ]);
    assert_eq!(r.code, 0, "{}", r.stderr);
    assert_eq!(text(&r.json(), &["file", "operation"]), "export");
    assert_eq!(run(&["import", &out_jpg, "-o", &s.path("j.pcnd")]).code, 0);

    // Bad invocations and hostile inputs.
    assert_eq!(run(&["render", &doc, "-o", &s.path("o.xyz")]).code, 2);
    assert_eq!(
        run(&[
            "render",
            &doc,
            "-o",
            &s.path("x.png"),
            "--allow-loss",
            "bogus"
        ])
        .code,
        2
    );
    assert_eq!(
        run(&[
            "render",
            &doc,
            "-o",
            &s.path("x.jpg"),
            "--allow-loss",
            "lossy-jpeg",
            "--quality",
            "0"
        ])
        .code,
        2
    );
    let junk = s.path("junk.png");
    fs::write(&junk, b"definitely not an image").unwrap();
    assert_eq!(run(&["import", &junk, "-o", &s.path("z.pcnd")]).code, 6);
    assert!(!Path::new(&s.path("z.pcnd")).exists());
    assert_eq!(
        run(&["import", &out_png, "-o", &back]).code,
        2,
        "existing output is protected"
    );
}

#[test]
fn apply_runs_a_v2_batch_with_strokes_fill_shapes_and_text() {
    let s = Scratch::new("v2");
    let (base, ops, out, png_path) = (
        s.path("base.pcnd"),
        s.path("ops.json"),
        s.path("out.pcnd"),
        s.path("o.png"),
    );
    run(&[
        "new",
        "--width",
        "64",
        "--height",
        "32",
        "--background",
        "#FFFFFFFF",
        "-o",
        &base,
    ]);
    let ins = run(&["inspect", &base, "--json"]).json();
    let state = text(&ins, &["ticket", "state_id"]).to_owned();
    let layer = ins
        .get("document")
        .unwrap()
        .get("layers")
        .unwrap()
        .as_array()
        .unwrap()[0]
        .get("id")
        .unwrap()
        .as_str()
        .unwrap()
        .to_owned();
    let l = format!(r#"{{"id":"{layer}"}}"#);
    fs::write(
        &ops,
        format!(
            r##"{{"schema":"paintcraft.batch/v2","atomic":true,"policy":"paintcraft.core/default-v1","base":{{"state_id":"{state}","publication_epoch":"0"}},"operations":[
              {{"op":"raster.shape","layer_ref":{l},"kind":"rect","from":{{"x":2,"y":2}},"to":{{"x":20,"y":12}},"fill":"#FF0000FF","stroke":{{"color":"#000000FF","size":1}},"mode":"replace"}},
              {{"op":"raster.stroke_path","layer_ref":{l},"points":[{{"x":0,"y":30}},{{"x":63,"y":30}}],"color":"#0000FFFF","mode":"replace","size":3,"brush":"round"}},
              {{"op":"raster.flood_fill","layer_ref":{l},"at":{{"x":40,"y":5}},"color":"#00FF00FF","mode":"replace","tolerance":0}},
              {{"op":"raster.text","layer_ref":{l},"at":{{"x":24,"y":14}},"text":"Hi","scale":1,"color":"#000000FF","mode":"replace"}}]}}"##
        ),
    )
    .unwrap();
    let r = run(&["apply", &base, "--operations", &ops, "-o", &out, "--json"]);
    assert_eq!(r.code, 0, "{} {}", r.stderr, r.stdout);
    assert_eq!(
        r.json().get("engine").unwrap().get("committed"),
        Some(&Value::Bool(true))
    );
    assert_eq!(run(&["render", &out, "-o", &png_path]).code, 0);
    let (w, h, px) = decode_png(&png_path);
    assert_eq!((w, h), (64, 32));
    let at = |x: usize, y: usize| px[(y * 64 + x) * 4..][..4].to_vec();
    assert_eq!(at(10, 7), [255, 0, 0, 255], "rectangle fill");
    assert_eq!(at(2, 2), [0, 0, 0, 255], "rectangle outline");
    assert_eq!(at(30, 30), [0, 0, 255, 255], "brush stroke");
    assert_eq!(
        at(60, 5),
        [0, 255, 0, 255],
        "flood fill of the white background"
    );
    assert_eq!(
        at(10, 22),
        [0, 255, 0, 255],
        "fill reached below the shapes"
    );
    assert_eq!(at(30, 25), [0, 255, 0, 255], "fill stops at the stroke");
    assert_eq!(at(24, 14)[..3], [0, 0, 0], "text pixel");
    // The same batch against a v1 schema string is refused, atomically.
    let v1 = fs::read_to_string(&ops)
        .unwrap()
        .replace("batch/v2", "batch/v1");
    fs::write(&ops, v1).unwrap();
    assert_eq!(
        run(&[
            "apply",
            &base,
            "--operations",
            &ops,
            "-o",
            &s.path("v1.pcnd")
        ])
        .code,
        2
    );
    assert!(!Path::new(&s.path("v1.pcnd")).exists());
}
