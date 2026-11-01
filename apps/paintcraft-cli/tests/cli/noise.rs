//! Actual CLI transport/rendering. Tiny literals come from the independent
//! numerical oracle; full-frame hashes are frozen qualified kernel observations,
//! not a second numerical oracle. No UI or latency guarantee is asserted here.
use super::catalog::read_catalog;
use super::*;
use paintcraft_document::{Document, POLICY};
use serde_json::{Value as Json, json};
use sha2::{Digest, Sha256};

fn descriptor(distribution: &str, mono: bool, amount: u16, seed: u16) -> Json {
    json!({"kind":"add-noise-rgb-v1","amount_hundredths":amount,
        "seed":seed,"monochrome":mono,"distribution":distribution})
}
fn apply_noise(doc: &Document, source: &str, ops: &str, output: &str, value: Json) {
    let packet = json!({"schema":"paintcraft.batch/catalog-v1","atomic":true,"policy":POLICY,
        "base":{"state_id":doc.state_id.to_string(),"publication_epoch":"0"},
        "required_filters":["add-noise-rgb-v1"],"operations":[{
            "op":"filter.add","layer_ref":{"id":doc.roots[0].to_string()},
            "ref":"noise","descriptor":value}]});
    fs::write(ops, serde_json::to_vec(&packet).unwrap()).unwrap();
    let result = run(&["apply", source, "--operations", ops, "-o", output, "--json"]);
    assert_eq!(result.code, 0, "{} {}", result.stdout, result.stderr);
}
fn render(source: &str, output: &str) -> Vec<u8> {
    let result = run(&[
        "render",
        source,
        "-o",
        output,
        "--allow-loss",
        "filters-baked",
        "--json",
    ]);
    assert_eq!(result.code, 0, "{} {}", result.stdout, result.stderr);
    decode_png(output).2
}
#[test]
fn noise_cli_literal_candidates_preserve_source_and_require_bake_consent() {
    let scratch = Scratch::new("noise-literal");
    for (distribution, expected) in [
        ("uniform", [255, 211, 0, 255, 51, 0, 97, 255]),
        ("gaussian", [47, 139, 0, 255, 0, 4, 46, 255]),
    ] {
        let doc = paintcraft_document::new_document(
            2,
            1,
            Some(paintcraft_core::types::Rgba([64, 16, 128, 255])),
            0,
            POLICY,
        )
        .unwrap();
        let source = scratch.path(&format!("{distribution}-source.pcnd"));
        let filtered = scratch.path(&format!("{distribution}-noise.pcnd"));
        let png = scratch.path(&format!("{distribution}.png"));
        let bytes = paintcraft_formats::pcnd::write_compact(&doc).unwrap();
        fs::write(&source, &bytes).unwrap();
        apply_noise(
            &doc,
            &source,
            &scratch.path("ops.json"),
            &filtered,
            descriptor(distribution, false, 100, 65535),
        );
        let saved = fs::read(&filtered).unwrap();
        assert_eq!(saved[4], 6);
        let reopened = read_catalog(&saved);
        assert_eq!(
            reopened.layers[&doc.roots[0]].raster,
            doc.layers[&doc.roots[0]].raster
        );
        let refused = run(&["render", &filtered, "-o", &png, "--json"]);
        assert_ne!(refused.code, 0);
        assert!(refused.stdout.contains("explicit consent to bake"));
        assert!(!Path::new(&png).exists());
        assert_eq!(render(&filtered, &png), expected);
        assert_eq!(fs::read(&filtered).unwrap(), saved);
        assert_eq!(fs::read(&source).unwrap(), bytes);
    }
}

#[test]
fn noise_cli_default_and_1080p_outputs_match_frozen_kernel_frames() {
    // build-final.json SHA1575b8862ce851c38e968a45b774d6f61f9a23013bc262134d8fbdb57e5b772f:
    // 2026-10-31 native Frame verification, subsequently matched by actual Wasm.
    // Source formula/absolute y match that previously frozen periodic-source probe.
    let scratch = Scratch::new("noise-full-frames");
    let cases = [
        (
            1000u32,
            486u32,
            "uniform",
            true,
            10u16,
            "8611b161d8825abac334c6a49e1862fcc9fb72205fb4b344116bfc7b5a2a9162",
        ),
        (
            1000u32,
            486u32,
            "uniform",
            false,
            10u16,
            "2d5508e33bf0b7c2d68b49167c86a2ee2ea2bc0070d2e52e80491f7d4cc7c7cf",
        ),
        (
            1000u32,
            486u32,
            "gaussian",
            true,
            10u16,
            "07f7d9ec563c7760589d1287515b61e1c8fe0f7405b28fdb6a5b288ca92bc020",
        ),
        (
            1000u32,
            486u32,
            "gaussian",
            false,
            10u16,
            "ae9ad7aad14fde88ae533306f6fc03ba3bcc334123f65f4fd5c57478f336eef7",
        ),
        (
            1920u32,
            1080u32,
            "uniform",
            true,
            10u16,
            "193dc4881de7a932e9b50058ba0b77cdaa82260944ac8e71965a5e4f3517f43e",
        ),
        (
            1920u32,
            1080u32,
            "uniform",
            false,
            10u16,
            "cc656042e5f8b56b0c75e90fea41d33341e8e90ea9fbd897816ed9649853e3d1",
        ),
        (
            1920u32,
            1080u32,
            "gaussian",
            true,
            10u16,
            "639aef02b15420de1a43000550916bdda021953c030f38e0bc7f4fa0e8f5bee5",
        ),
        (
            1920u32,
            1080u32,
            "gaussian",
            false,
            10u16,
            "79add844f43d3c8914b1e0e23d8a9b69253acb7a3146d4bb9a6c8eeb6f8a27f4",
        ),
        (
            1000u32,
            486u32,
            "gaussian",
            false,
            0u16,
            "2075145d0dd5851b0dc06fbe77de0a52fe379892e0fd802bc6487a71e0d68160",
        ),
        (
            1920u32,
            1080u32,
            "gaussian",
            false,
            0u16,
            "536b8daf553df09085aafdc073389781bc8420539ec0672cd912b6b16137ea23",
        ),
    ];
    for (index, (width, height, distribution, mono, amount, expected)) in
        cases.into_iter().enumerate()
    {
        let mut rgba = Vec::with_capacity((width * height * 4) as usize);
        for y in 0..height {
            for x in 0..width {
                rgba.extend_from_slice(&[
                    ((17 * x + 31 * y + 23) % 256) as u8,
                    ((29 * x + 11 * y + 71) % 256) as u8,
                    ((7 * x + 43 * y + 193) % 256) as u8,
                    255,
                ]);
            }
        }
        let doc =
            paintcraft_document::import_image(width, height, &rgba, "noise-frame-probe").unwrap();
        drop(rgba);
        let source = scratch.path(&format!("source-{index}.pcnd"));
        let filtered = scratch.path(&format!("noise-{index}.pcnd"));
        let png = scratch.path(&format!("noise-{index}.png"));
        let bytes = paintcraft_formats::pcnd::write_compact(&doc).unwrap();
        fs::write(&source, &bytes).unwrap();
        apply_noise(
            &doc,
            &source,
            &scratch.path("ops.json"),
            &filtered,
            descriptor(distribution, mono, amount, 1),
        );
        let start = std::time::Instant::now();
        let rendered = render(&filtered, &png);
        eprintln!(
            "Noise CLI full frame {index}: {width}x{height} {distribution} mono={mono} amount={amount}, process/render/write/decode {:?}",
            start.elapsed()
        );
        assert_eq!(rendered.len(), (width * height * 4) as usize);
        assert_eq!(
            paintcraft_core::types::policy::hex(&Sha256::digest(&rendered)),
            expected,
            "{index}"
        );
        assert_eq!(fs::read(&source).unwrap(), bytes);
    }
}
