use serde_json::{Value, json};
use std::{
    fs,
    path::Path,
    process::{Command, Output},
};
#[path = "support/mcp_client.rs"]
mod mcp_client;
use mcp_client::Client;
const SOURCE: &[u8] = include_bytes!(
    "../../../crates/services/paintcraft-formats/tests/dds-mip-fixtures/dx10-rgba.dds"
);
const TOP: &[u8] =
    include_bytes!("../../../crates/services/paintcraft-formats/tests/dds-mip-fixtures/top.rgba");
const LOSS: &str = "dds-other-mips-not-imported";
fn run(args: &[&str]) -> Output {
    Command::new(env!("CARGO_BIN_EXE_paintcraft-cli"))
        .args(args)
        .output()
        .unwrap()
}
fn ok(output: Output) -> Value {
    assert!(
        output.status.success(),
        "{} {}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    serde_json::from_slice(&output.stdout).unwrap()
}
fn pixels(path: &Path) {
    let doc = paintcraft_formats::pcnd::read(&fs::read(path).unwrap()).unwrap();
    assert_eq!(doc.canvas, (7, 5));
    for (i, p) in TOP.as_chunks::<4>().0.iter().enumerate() {
        assert_eq!(
            doc.layers[&doc.roots[0]]
                .raster
                .get((i % 7) as i64, (i / 7) as i64)
                .0,
            *p
        );
    }
}
#[test]
fn actual_cli_mcp_dds_loss_consent_pixels_and_retained_source_guards() {
    let root = std::env::temp_dir().join(format!("paintcraft-dds-cli-{}", std::process::id()));
    fs::create_dir_all(&root).unwrap();
    for name in ["mips.dds", "extensionless"] {
        let src = root.join(name);
        fs::write(&src, SOURCE).unwrap();
        let input = src.to_str().unwrap();
        let out = root.join(format!("{name}.pcnd"));
        let dest = out.to_str().unwrap();
        fs::write(&out, b"existing destination").unwrap();
        for loss in [None, Some("made-up")] {
            let mut args = vec![
                "import",
                input,
                "-o",
                dest,
                "--overwrite",
                "--frontend",
                "studio",
                "--json",
            ];
            if let Some(loss) = loss {
                args.extend(["--allow-loss", loss]);
            }
            assert!(!run(&args).status.success());
            assert_eq!(fs::read(&out).unwrap(), b"existing destination");
        }
        let result = ok(run(&[
            "import",
            input,
            "-o",
            dest,
            "--overwrite",
            "--frontend",
            "studio",
            "--allow-loss",
            LOSS,
            "--json",
        ]));
        assert_eq!(result["file"]["losses_acknowledged"], json!([LOSS]));
        pixels(&out);
        assert!(
            !run(&[
                "import",
                input,
                "-o",
                dest,
                "--overwrite",
                "--frontend",
                "classic-v11",
                "--allow-loss",
                LOSS
            ])
            .status
            .success()
        );
        pixels(&out);
        let alias = root.join(format!("{name}-alias.pcnd"));
        fs::hard_link(&src, &alias).unwrap();
        assert!(
            !run(&[
                "import",
                input,
                "-o",
                alias.to_str().unwrap(),
                "--overwrite",
                "--allow-loss",
                LOSS
            ])
            .status
            .success()
        );
        assert_eq!(fs::read(&src).unwrap(), SOURCE);
        // Existing paste command must ask before discarding lower levels as well.
        let pasted = root.join(format!("{name}-pasted.pcnd"));
        assert!(
            !run(&[
                "paste-image",
                dest,
                input,
                "-o",
                pasted.to_str().unwrap(),
                "--frontend",
                "studio"
            ])
            .status
            .success()
        );
        assert!(!pasted.exists());
        ok(run(&[
            "paste-image",
            dest,
            input,
            "-o",
            pasted.to_str().unwrap(),
            "--frontend",
            "studio",
            "--allow-loss",
            LOSS,
            "--json",
        ]));
        pixels(&pasted);
        assert!(
            !run(&[
                "paste-image",
                dest,
                input,
                "-o",
                alias.to_str().unwrap(),
                "--overwrite",
                "--frontend",
                "studio",
                "--allow-loss",
                LOSS
            ])
            .status
            .success()
        );
        assert_eq!(fs::read(&src).unwrap(), SOURCE);

        let mut c = Client::at(Path::new(env!("CARGO_BIN_EXE_paintcraft-cli")));
        c.ok("create_canvas", json!({"width":2,"height":2}));
        c.ok("paint_stroke",json!({"points":[{"x":0,"y":0},{"x":1,"y":1}],"color":"#123456FF","size":1,"brush":"round"}));
        let before = c.ok("inspect_document", json!({}));
        for args in [
            json!({"path":src,"allow_losses":[LOSS]}),
            json!({"path":src,"discard_changes":true}),
            json!({"path":src,"discard_changes":true,"allow_losses":["made-up"]}),
        ] {
            assert_eq!(c.call("import_image", args)["isError"], true);
            assert_eq!(c.ok("inspect_document", json!({})), before);
        }
        let imported = c.ok(
            "import_image",
            json!({"path":src,"discard_changes":true,"allow_losses":[LOSS]}),
        );
        assert_eq!(imported["dirty"], true);
        let saved = root.join(format!("{name}-mcp.pcnd"));
        c.ok("save_project", json!({"path":saved}));
        pixels(&saved);
        let moved = root.join(format!("{name}-renamed.dds"));
        fs::rename(&src, &moved).unwrap();
        let alias2 = root.join(format!("{name}-mcp-alias.pcnd"));
        fs::hard_link(&moved, &alias2).unwrap();
        assert_eq!(
            c.call("save_project", json!({"path":alias2,"overwrite":true}))["isError"],
            true
        );
        assert_eq!(fs::read(&moved).unwrap(), SOURCE);
    }
    fs::remove_dir_all(root).unwrap();
}
