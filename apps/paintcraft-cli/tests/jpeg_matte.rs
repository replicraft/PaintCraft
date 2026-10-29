//! Real CLI/stdio MCP JPEG matte option delivery and pre-render refusal.
use paintcraft_formats::{image, pcnd};
use serde_json::json;
use std::{
    fs,
    path::{Path, PathBuf},
    process::Command,
};
#[path = "support/mcp_client.rs"]
mod mcp_client;
struct Folder {
    path: PathBuf,
    retained: bool,
}
impl Folder {
    fn new() -> Self {
        let selected = std::env::var_os("PAINTCRAFT_JPEG_MATTE_EVIDENCE");
        let retained = selected.is_some();
        let path = selected.map(PathBuf::from).unwrap_or_else(|| {
            std::env::temp_dir().join(format!("paintcraft-jpeg-matte-{}", uuid::Uuid::new_v4()))
        });
        fs::create_dir_all(&path).unwrap();
        Self { path, retained }
    }
}
impl Drop for Folder {
    fn drop(&mut self) {
        if !self.retained {
            fs::remove_dir_all(&self.path).unwrap();
        }
    }
}
fn source(folder: &Folder) -> PathBuf {
    let rgba: Vec<u8> = (0..64).flat_map(|_| [17, 92, 231, 128]).collect();
    let document = paintcraft_document::import_image(8, 8, &rgba, "jpeg-matte-consumer").unwrap();
    let path = folder.path.join("source.pcnd");
    fs::write(&path, pcnd::write_compact(&document).unwrap()).unwrap();
    path
}
fn render(input: &Path, output: &Path, matte: Option<&str>, allow: bool) -> std::process::Output {
    let mut cmd = Command::new(env!("CARGO_BIN_EXE_paintcraft-cli"));
    cmd.arg("render")
        .arg(input)
        .arg("-o")
        .arg(output)
        .args(["--json", "--overwrite"]);
    if let Some(matte) = matte {
        cmd.args(["--jpeg-matte", matte]);
    }
    if allow {
        cmd.args(["--allow-loss", "lossy-jpeg,alpha-discarded"]);
    }
    cmd.output().unwrap()
}
fn samples(path: &Path, expected: [u8; 3]) {
    let decoded = image::decode(&fs::read(path).unwrap()).unwrap();
    assert_eq!((decoded.width, decoded.height), (8, 8));
    assert!(decoded.rgba.as_chunks::<4>().0.iter().all(|pixel| {
        pixel[..3]
            .iter()
            .zip(expected)
            .all(|(a, b)| a.abs_diff(b) <= 2)
            && pixel[3] == 255
    }));
}

#[test]
fn actual_cli_and_mcp_matte_match_authored_samples_and_preserve_white_default_and_project() {
    let folder = Folder::new();
    let source = source(&folder);
    let before = fs::read(&source).unwrap();
    let mut mcp = mcp_client::Client::at(Path::new(env!("CARGO_BIN_EXE_paintcraft-cli")));
    mcp.ok("open_project", json!({"path":source}));
    let state = mcp.ok("inspect_document", json!({}));
    for (name, matte, expected) in [
        ("color", Some("20A0E0"), [24, 126, 228]),
        ("black", Some("000000"), [9, 46, 116]),
        ("white", Some("ffffff"), [136, 173, 243]),
        ("default", None, [136, 173, 243]),
    ] {
        let cli = folder.path.join(format!("cli-{name}.jpg"));
        let result = render(&source, &cli, matte, true);
        assert!(
            result.status.success(),
            "{} {}",
            String::from_utf8_lossy(&result.stdout),
            String::from_utf8_lossy(&result.stderr)
        );
        samples(&cli, expected);
        let remote = folder.path.join(format!("mcp-{name}.jpg"));
        let mut args =
            json!({"path":remote,"overwrite":true,"allow_losses":["lossy-jpeg","alpha-discarded"]});
        if let Some(value) = matte {
            args["jpeg_matte"] = value.into();
        }
        mcp.ok("export_image", args);
        samples(&remote, expected);
        assert_eq!(fs::read(cli).unwrap(), fs::read(remote).unwrap());
        assert_eq!(mcp.ok("inspect_document", json!({})), state);
    }
    assert_eq!(
        fs::read(folder.path.join("cli-white.jpg")).unwrap(),
        fs::read(folder.path.join("cli-default.jpg")).unwrap()
    );
    assert_eq!(fs::read(source).unwrap(), before);
}

#[test]
fn invalid_matte_wrong_format_and_missing_loss_consent_leave_existing_destination_and_session_untouched()
 {
    let folder = Folder::new();
    let source = source(&folder);
    let mut mcp = mcp_client::Client::at(Path::new(env!("CARGO_BIN_EXE_paintcraft-cli")));
    mcp.ok("open_project", json!({"path":source}));
    let state = mcp.ok("inspect_document", json!({}));
    for (suffix, matte, allow) in [
        ("png", "ffffff", true),
        ("jpg", "ffffff00", true),
        ("jpg", "20a0e0", false),
    ] {
        let output = folder.path.join(format!("refused.{suffix}"));
        fs::write(&output, b"existing destination").unwrap();
        let result = render(&source, &output, Some(matte), allow);
        assert!(!result.status.success());
        let diagnostic = format!(
            "{} {}",
            String::from_utf8_lossy(&result.stdout),
            String::from_utf8_lossy(&result.stderr)
        );
        if suffix == "png" {
            assert!(diagnostic.contains("requires JPEG"), "{diagnostic}");
        } else if matte.len() != 6 {
            assert!(
                diagnostic.contains("six RGB hexadecimal digits"),
                "{diagnostic}"
            );
        } else {
            assert!(
                diagnostic.contains("lossy-jpeg") && diagnostic.contains("alpha-discarded"),
                "{diagnostic}"
            );
        }
        let mut args = json!({"path":output,"jpeg_matte":matte,"overwrite":true,"allow_losses":[]});
        if allow {
            args["allow_losses"] = json!(["lossy-jpeg", "alpha-discarded"]);
        }
        assert_eq!(mcp.call("export_image", args)["isError"], true);
        assert_eq!(fs::read(output).unwrap(), b"existing destination");
        assert_eq!(mcp.ok("inspect_document", json!({})), state);
    }
}
