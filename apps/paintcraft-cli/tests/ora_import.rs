//! Existing import command and real stdio MCP, with actual ORA archive bytes.
use serde_json::{Value, json};
use std::{
    fs,
    path::Path,
    process::{Command, Output},
};
#[path = "support/mcp_client.rs"]
mod mcp_client;
use mcp_client::Client;
const GROUPED: &[u8] = include_bytes!(
    "../../../crates/services/paintcraft-formats/tests/ora-group-fixtures/neutral.ora"
);
const FLAT: &[u8] =
    include_bytes!("../../../crates/services/paintcraft-formats/tests/ora-group-fixtures/flat.ora");
const MERGED: &[u8] = include_bytes!(
    "../../../crates/services/paintcraft-formats/tests/ora-group-fixtures/merged.rgba"
);
const LOSS: &str = "ora-group-structure-not-retained";
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
fn check(path: &Path) {
    let doc = paintcraft_formats::pcnd::read(&fs::read(path).unwrap()).unwrap();
    assert_eq!(doc.canvas, (3, 2));
    assert_eq!(doc.roots.len(), 6);
    assert_eq!(paintcraft_render::render_flat(&doc).unwrap(), MERGED);
    assert!(!doc.layers[&doc.roots[2]].visible);
    assert_eq!(doc.layers[&doc.roots[1]].raster.get(0, 1).0, [81, 8, 13, 0]);
}
#[test]
fn actual_cli_and_mcp_group_copies_require_loss_consent_and_retain_source_guards_after_save() {
    let root = std::env::temp_dir().join(format!("paintcraft-ora-cli-{}", std::process::id()));
    fs::create_dir_all(&root).unwrap();
    for name in ["groups.ora", "extensionless"] {
        let input = root.join(name);
        fs::write(&input, GROUPED).unwrap();
        let source = input.to_str().unwrap();
        let project = root.join(format!("{name}.pcnd"));
        let dest = project.to_str().unwrap();
        fs::write(&project, b"old destination").unwrap();
        for allowed in [None, Some("made-up")] {
            let mut args = vec![
                "import",
                source,
                "-o",
                dest,
                "--overwrite",
                "--frontend",
                "studio",
                "--json",
            ];
            if let Some(code) = allowed {
                args.extend(["--allow-loss", code]);
            }
            assert!(!run(&args).status.success());
            assert_eq!(fs::read(&project).unwrap(), b"old destination");
        }
        let receipt = ok(run(&[
            "import",
            source,
            "-o",
            dest,
            "--overwrite",
            "--frontend",
            "studio",
            "--allow-loss",
            LOSS,
            "--json",
        ]));
        assert_eq!(receipt["file"]["losses_acknowledged"], json!([LOSS]));
        check(&project);
        let stable = fs::read(&project).unwrap();
        assert!(
            !run(&[
                "import",
                source,
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
        assert_eq!(fs::read(&project).unwrap(), stable);
        let alias = root.join(format!("{name}-alias.pcnd"));
        fs::hard_link(&input, &alias).unwrap();
        assert!(
            !run(&[
                "import",
                source,
                "-o",
                alias.to_str().unwrap(),
                "--overwrite",
                "--allow-loss",
                LOSS
            ])
            .status
            .success()
        );
        assert_eq!(fs::read(&input).unwrap(), GROUPED);
        let mut c = Client::at(Path::new(env!("CARGO_BIN_EXE_paintcraft-cli")));
        c.ok("create_canvas", json!({"width":2,"height":2}));
        c.ok("paint_stroke",json!({"points":[{"x":0,"y":0},{"x":1,"y":1}],"color":"#123456FF","size":1,"brush":"round"}));
        let before = c.ok("inspect_document", json!({}));
        for args in [
            json!({"path":input,"allow_losses":[LOSS]}),
            json!({"path":input,"discard_changes":true}),
            json!({"path":input,"discard_changes":true,"allow_losses":["made-up"]}),
        ] {
            assert_eq!(c.call("import_image", args)["isError"], true);
            assert_eq!(c.ok("inspect_document", json!({})), before);
        }
        for bad in [include_bytes!("../../../crates/services/paintcraft-formats/tests/ora-group-fixtures/hidden-isolate.ora").as_slice(),include_bytes!("../../../crates/services/paintcraft-formats/tests/ora-group-fixtures/different-projection.ora"),b"PK\x03\x04unknown ZIP"] {
            let path=root.join("refused");fs::write(&path,bad).unwrap();assert_eq!(c.call("import_image",json!({"path":path,"discard_changes":true,"allow_losses":[LOSS]}))["isError"],true);assert_eq!(c.ok("inspect_document",json!({})),before);
            assert!(!run(&["import",path.to_str().unwrap(),"-o",dest,"--overwrite","--allow-loss",LOSS]).status.success());assert_eq!(fs::read(&project).unwrap(),stable);
        }
        let adopted = c.ok(
            "import_image",
            json!({"path":input,"discard_changes":true,"allow_losses":[LOSS]}),
        );
        assert_eq!(adopted["dirty"], true);
        assert_eq!(adopted["conversion"]["losses_acknowledged"], json!([LOSS]));
        assert_eq!(adopted["conversion"]["preserving_save"], false);
        let saved = root.join(format!("{name}-mcp.pcnd"));
        c.ok("save_project", json!({"path":saved}));
        check(&saved);
        let moved = root.join(format!("{name}-renamed.ora"));
        fs::rename(&input, &moved).unwrap();
        let source_alias = root.join(format!("{name}-mcp-alias.pcnd"));
        fs::hard_link(&moved, &source_alias).unwrap();
        assert_eq!(
            c.call(
                "save_project",
                json!({"path":source_alias,"overwrite":true})
            )["isError"],
            true
        );
        let png_copy = root.join(format!("{name}-unrelated.png"));
        c.ok(
            "export_image",
            json!({"path":png_copy,"allow_losses":paintcraft_formats::export::KNOWN_LOSSES}),
        );
        assert!(png_copy.is_file());
        let png_alias = root.join(format!("{name}-mcp-alias.png"));
        fs::hard_link(&moved, &png_alias).unwrap();
        assert_eq!(c.call("export_image",json!({"path":png_alias,"overwrite":true,"allow_losses":paintcraft_formats::export::KNOWN_LOSSES}))["isError"],true);
        let unrelated = root.join(format!("{name}-unrelated.pcnd"));
        c.ok("save_project", json!({"path":unrelated}));
        check(&unrelated);
        assert_eq!(fs::read(&moved).unwrap(), GROUPED);
    }
    fs::remove_dir_all(root).unwrap();
}

#[cfg(unix)]
fn stream_import(path: &Path, output: &Path, bytes: &[u8], allow_loss: bool) -> Output {
    use rustix::fs::{CWD, Mode, OFlags, openat};
    use std::{
        io::Write,
        process::{Child, Stdio},
        time::{Duration, Instant},
    };
    struct Process(Option<Child>);
    impl Drop for Process {
        fn drop(&mut self) {
            if let Some(mut child) = self.0.take() {
                let _ = child.kill();
                let _ = child.wait();
            }
        }
    }
    assert!(Command::new("mkfifo").arg(path).status().unwrap().success());
    let mut command = Command::new(env!("CARGO_BIN_EXE_paintcraft-cli"));
    command.args([
        "import",
        path.to_str().unwrap(),
        "-o",
        output.to_str().unwrap(),
        "--frontend",
        "studio",
        "--json",
    ]);
    if allow_loss {
        command.args(["--allow-loss", LOSS]);
    }
    let mut process = Process(Some(
        command
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .unwrap(),
    ));
    let until = Instant::now() + Duration::from_secs(10);
    let mut writer = loop {
        match openat(
            CWD,
            path,
            OFlags::WRONLY | OFlags::NONBLOCK | OFlags::CLOEXEC,
            Mode::empty(),
        ) {
            Ok(fd) => break fs::File::from(fd),
            Err(rustix::io::Errno::NXIO) => {
                assert!(process.0.as_mut().unwrap().try_wait().unwrap().is_none());
                assert!(Instant::now() < until);
                std::thread::sleep(Duration::from_millis(2));
            }
            Err(error) => panic!("pipe: {error}"),
        }
    };
    writer.write_all(bytes).unwrap();
    drop(writer);
    process.0.take().unwrap().wait_with_output().unwrap()
}
#[test]
#[cfg(unix)]
fn legacy_flat_ora_pipe_stays_eligible_and_new_groups_never_use_the_stream_profile() {
    let root = std::env::temp_dir().join(format!("paintcraft-ora-pipe-{}", std::process::id()));
    fs::create_dir_all(&root).unwrap();
    let flat = root.join("flat.pcnd");
    ok(stream_import(&root.join("flat.ora"), &flat, FLAT, false));
    check(&flat);
    let refused = root.join("groups.pcnd");
    assert!(
        !stream_import(&root.join("groups.ora"), &refused, GROUPED, true)
            .status
            .success()
    );
    assert!(!refused.exists());
    fs::remove_dir_all(root).unwrap();
}
