use super::*;

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
fn import_loss_codes_can_be_acknowledged_without_weakening_export_validation() {
    let scratch = Scratch::new("import-depth-loss");
    let input = scratch.path("deep.png");
    let output = scratch.path("deep.pcnd");
    let mut bytes = Vec::new();
    let mut encoder = png::Encoder::new(&mut bytes, 1, 1);
    encoder.set_color(png::ColorType::Rgba);
    encoder.set_depth(png::BitDepth::Sixteen);
    encoder
        .write_header()
        .unwrap()
        .write_image_data(&[0x12, 0x34, 0x56, 0x78, 0x9a, 0xbc, 0xff, 0xff])
        .unwrap();
    fs::write(&input, bytes).unwrap();
    let rejected = run(&["import", &input, "-o", &output, "--json"]);
    assert_eq!(rejected.code, 3);
    assert!(!Path::new(&output).exists());
    let accepted = run(&[
        "import",
        &input,
        "-o",
        &output,
        "--allow-loss",
        "depth-16-reduced-to-8",
        "--json",
    ]);
    assert_eq!(accepted.code, 0, "{}", accepted.stderr);
    assert_eq!(
        accepted
            .json()
            .get("file")
            .unwrap()
            .get("losses_acknowledged")
            .unwrap(),
        &Value::Array(vec![Value::str("depth-16-reduced-to-8")])
    );
    assert_eq!(
        run(&[
            "render",
            &output,
            "-o",
            &scratch.path("out.png"),
            "--allow-loss",
            "depth-16-reduced-to-8"
        ])
        .code,
        2
    );
}
