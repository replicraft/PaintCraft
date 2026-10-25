use super::*;
use paintcraft_core::types::{Rgba, policy::POLICY};
use paintcraft_document::new_document;
use sha2::{Digest, Sha256};
fn fixture(
    scratch: &Scratch,
) -> (
    String,
    String,
    paintcraft_document::Document,
    Vec<u8>,
    Vec<u8>,
) {
    let input = scratch.path("base.pcnd");
    let image = scratch.path("source.png");
    let doc = new_document(4, 3, Some(Rgba([255; 4])), 0, POLICY).unwrap();
    let original = paintcraft_formats::pcnd::write(&doc).unwrap();
    let rgba = vec![
        17, 44, 99, 0, 255, 0, 7, 127, 90, 80, 70, 255, 0, 0, 0, 0, 1, 2, 3, 1, 4, 5, 6, 200,
    ];
    let png = paintcraft_formats::image::encode(
        paintcraft_formats::image::ImageFormat::Png,
        3,
        2,
        &rgba,
        90,
    )
    .unwrap();
    fs::write(&input, &original).unwrap();
    fs::write(&image, &png).unwrap();
    (input, image, doc, original, rgba)
}
#[test]
fn real_cli_pastes_exact_raw_rgba_as_a_layer_with_explicit_canvas_policy() {
    let scratch = Scratch::new("paste-image-matrix");
    let (input, image, doc, original, rgba) = fixture(&scratch);
    let source = fs::read(&image).unwrap();
    for (x, y, grow, canvas) in [
        ("-1", "1", false, (4, 3)),
        ("3", "2", true, (6, 4)),
        ("10", "10", false, (4, 3)),
    ] {
        let output = scratch.path(&format!("paste-{x}-{y}.pcnd"));
        let mut args = vec![
            "paste-image",
            &input,
            &image,
            "--x",
            x,
            "--y",
            y,
            "--name",
            "貼り付け layer",
            "-o",
            &output,
            "--json",
        ];
        if grow {
            args.push("--expand-canvas");
        }
        let reply = run(&args);
        assert_eq!(reply.code, 0, "{} {}", reply.stdout, reply.stderr);
        let result = reply.json();
        let id = text(&result, &["engine", "aliases", "pasted"])
            .parse()
            .unwrap();
        let reopened = paintcraft_formats::pcnd::read(&fs::read(&output).unwrap()).unwrap();
        assert_eq!(reopened.canvas, canvas);
        assert_eq!(reopened.document_id, doc.document_id);
        assert_eq!(reopened.roots, vec![doc.roots[0], id]);
        assert_eq!(reopened.layers[&doc.roots[0]], doc.layers[&doc.roots[0]]);
        let layer = &reopened.layers[&id];
        assert_eq!(layer.name, "貼り付け layer");
        assert_eq!(layer.origin, (x.parse().unwrap(), y.parse().unwrap()));
        for y in 0..2 {
            for x in 0..3 {
                let offset = ((y * 3 + x) * 4) as usize;
                assert_eq!(&layer.raster.get(x, y).0, &rgba[offset..offset + 4]);
            }
        }
        assert_eq!(fs::read(&input).unwrap(), original);
        assert_eq!(fs::read(&image).unwrap(), source);
    }
}
#[test]
fn real_cli_hash_guards_invalid_growth_and_source_alias_leave_existing_files_untouched() {
    let scratch = Scratch::new("paste-image-guards");
    let (input, image, _, original, _) = fixture(&scratch);
    let source = fs::read(&image).unwrap();
    let output = scratch.path("output.pcnd");
    let previous = b"preserved destination";
    fs::write(&output, previous).unwrap();
    let incorrect = "0".repeat(64);
    for flag in ["--expect-input-sha256", "--expect-image-sha256"] {
        let reply = run(&[
            "paste-image",
            &input,
            &image,
            flag,
            &incorrect,
            "-o",
            &output,
            "--overwrite",
            "--json",
        ]);
        assert_eq!(reply.code, 4, "{} {}", reply.stdout, reply.stderr);
        assert_eq!(
            text(&reply.json(), &["engine", "code"]),
            "conflict.stale_base"
        );
        assert_eq!(fs::read(&output).unwrap(), previous);
    }
    let rejected = run(&[
        "paste-image",
        &input,
        &image,
        "--x",
        "-1",
        "--expand-canvas",
        "-o",
        &output,
        "--overwrite",
        "--json",
    ]);
    assert_eq!(rejected.code, 2, "{} {}", rejected.stdout, rejected.stderr);
    assert_eq!(fs::read(&output).unwrap(), previous);
    let alias = run(&[
        "paste-image",
        &input,
        &image,
        "-o",
        &image,
        "--overwrite",
        "--json",
    ]);
    assert_eq!(alias.code, 2, "{} {}", alias.stdout, alias.stderr);
    assert_eq!(fs::read(&image).unwrap(), source);
    assert_eq!(fs::read(&input).unwrap(), original);
    let input_hash = paintcraft_document::hex(&Sha256::digest(&original));
    let image_hash = paintcraft_document::hex(&Sha256::digest(&source));
    let accepted = run(&[
        "paste-image",
        &input,
        &image,
        "--expect-input-sha256",
        &input_hash,
        "--expect-image-sha256",
        &image_hash,
        "-o",
        &output,
        "--overwrite",
        "--json",
    ]);
    assert_eq!(accepted.code, 0, "{} {}", accepted.stdout, accepted.stderr);
}
#[test]
fn real_cli_requires_loss_consent_for_depth_reduction_before_writing() {
    let scratch = Scratch::new("paste-image-losses");
    let (input, _, _, original, _) = fixture(&scratch);
    let image = scratch.path("depth.png");
    let output = scratch.path("output.pcnd");
    let mut png = Vec::new();
    {
        let mut encoder = png::Encoder::new(&mut png, 1, 1);
        encoder.set_color(png::ColorType::Grayscale);
        encoder.set_depth(png::BitDepth::Sixteen);
        encoder
            .write_header()
            .unwrap()
            .write_image_data(&[0x12, 0x34])
            .unwrap();
    }
    fs::write(&image, &png).unwrap();
    fs::write(&output, b"existing").unwrap();
    let denied = run(&[
        "paste-image",
        &input,
        &image,
        "-o",
        &output,
        "--overwrite",
        "--json",
    ]);
    assert_eq!(denied.code, 3, "{} {}", denied.stdout, denied.stderr);
    assert_eq!(fs::read(&output).unwrap(), b"existing");
    let accepted = run(&[
        "paste-image",
        &input,
        &image,
        "--allow-loss",
        "depth-16-reduced-to-8",
        "-o",
        &output,
        "--overwrite",
        "--json",
    ]);
    assert_eq!(accepted.code, 0, "{} {}", accepted.stdout, accepted.stderr);
    let document = paintcraft_formats::pcnd::read(&fs::read(&output).unwrap()).unwrap();
    let layer = &document.layers[document.roots.last().unwrap()];
    assert_eq!(layer.raster.get(0, 0), Rgba([18, 18, 18, 255]));
    assert_eq!(fs::read(&input).unwrap(), original);
    assert_eq!(fs::read(&image).unwrap(), png);
}

#[test]
fn paste_image_discovery_reports_implementation_without_release_qualification() {
    let reply = run(&["capabilities", "--json"]);
    assert_eq!(reply.code, 0);
    let value = reply.json();
    let rows = value.get("capabilities").unwrap().as_array().unwrap();
    for capability in ["image.rgba8.paste-layer", "image.rgba8.paste-onto"] {
        let row = rows
            .iter()
            .find(|row| row.get("capability").and_then(Value::as_str) == Some(capability))
            .expect("Paste capability");
        assert_eq!(row.get("available"), Some(&Value::Bool(true)));
        assert_eq!(row.get("enabled"), Some(&Value::Bool(true)));
        assert_eq!(row.get("release_qualified"), Some(&Value::Bool(false)));
        assert_eq!(text(row, &["operation"]), "paste-image");
    }
}

#[test]
fn real_cli_pastes_onto_an_existing_layer_with_explicit_blending_and_growth() {
    let scratch = Scratch::new("paste-onto-layer");
    let (input, image, doc, original, rgba) = fixture(&scratch);
    let target = doc.roots[0].to_string();
    for (blend, x, y, grow, canvas) in [
        ("replace", "-1", "1", false, (4, 3)),
        ("source-over", "1", "1", false, (4, 3)),
        ("replace", "3", "2", true, (6, 4)),
    ] {
        let output = scratch.path(&format!("onto-{blend}-{x}.pcnd"));
        let mut args = vec![
            "paste-image",
            &input,
            &image,
            "--onto-layer",
            &target,
            "--blend",
            blend,
            "--x",
            x,
            "--y",
            y,
            "-o",
            &output,
            "--json",
        ];
        if grow {
            args.push("--expand-canvas");
        }
        let reply = run(&args);
        assert_eq!(reply.code, 0, "{} {}", reply.stdout, reply.stderr);
        assert_eq!(
            text(&reply.json(), &["engine", "aliases", "pasted"]),
            target
        );
        let reopened = paintcraft_formats::pcnd::read(&fs::read(&output).unwrap()).unwrap();
        assert_eq!(reopened.roots, doc.roots);
        assert_eq!(reopened.canvas, canvas);
        let layer = &reopened.layers[&doc.roots[0]];
        assert_eq!(layer.name, doc.layers[&doc.roots[0]].name);
        let (ox, oy) = (x.parse::<i64>().unwrap(), y.parse::<i64>().unwrap());
        for py in 0..2 {
            for px in 0..3 {
                let at = (py * 3 + px) * 4;
                let src = Rgba(rgba[at..at + 4].try_into().unwrap());
                let (dx, dy) = (ox + px as i64, oy + py as i64);
                let dst = if doc.canvas_rect().contains(dx, dy) {
                    Rgba([255; 4])
                } else {
                    Rgba::TRANSPARENT
                };
                let expected = if blend == "replace" {
                    src
                } else {
                    src.source_over(dst)
                };
                assert_eq!(
                    layer.raster.get(dx - layer.origin.0, dy - layer.origin.1),
                    expected
                );
            }
        }
        assert_eq!(fs::read(&input).unwrap(), original);
    }
    // The implicit alpha policy is source-over; layer names are preserved.
    let output = scratch.path("default-onto.pcnd");
    let reply = run(&[
        "paste-image",
        &input,
        &image,
        "--onto-layer",
        &target,
        "-o",
        &output,
    ]);
    assert_eq!(reply.code, 0, "{} {}", reply.stdout, reply.stderr);
    let reopened = paintcraft_formats::pcnd::read(&fs::read(&output).unwrap()).unwrap();
    assert_eq!(
        reopened.layers[&doc.roots[0]].raster.get(0, 0),
        Rgba([255; 4])
    );
}
#[test]
fn cli_existing_layer_flags_reject_ambiguous_or_invalid_targets_without_output() {
    let scratch = Scratch::new("paste-onto-invalid");
    let (input, image, _, original, _) = fixture(&scratch);
    let output = scratch.path("never.pcnd");
    for extra in [
        vec!["--onto-layer", "not-a-uuid"],
        vec!["--onto-layer", "00000000-0000-0000-0000-000000000000"],
        vec!["--blend", "replace"],
        vec![
            "--onto-layer",
            "00000000-0000-0000-0000-000000000000",
            "--name",
            "ignored",
        ],
    ] {
        let mut args = vec!["paste-image", &input, &image, "-o", &output, "--json"];
        args.extend(extra);
        let reply = run(&args);
        assert_ne!(reply.code, 0, "unexpected success {}", reply.stdout);
        assert!(!std::path::Path::new(&output).exists());
        assert_eq!(fs::read(&input).unwrap(), original);
    }
}
