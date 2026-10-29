//! Actual positive specialty copies preserve authoring metadata and require intent.
use paintcraft_formats::{dcx, dds, icns, icons, msp, pcnd};
use serde_json::{Value, json};
use std::{
    fs,
    path::{Path, PathBuf},
    process::Command,
};
#[path = "support/mcp_client.rs"]
mod mcp_client;
const RGBA: [u8; 16] = [
    0, 0, 0, 255, 255, 255, 255, 255, 255, 255, 255, 255, 0, 0, 0, 255,
];
struct Folder(PathBuf);
impl Drop for Folder {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}
fn folder() -> Folder {
    let p = std::env::temp_dir().join(format!(
        "paintcraft-specialty-metadata-{}",
        uuid::Uuid::new_v4()
    ));
    fs::create_dir(&p).unwrap();
    Folder(p)
}
fn project() -> Vec<u8> {
    let mut doc =
        paintcraft_document::import_image(2, 2, &RGBA, "specialty-metadata-authored-opaque")
            .unwrap();
    doc.source_metadata = pcnd::read(include_bytes!(
        "../../../crates/services/paintcraft-formats/tests/pcnd-metadata-fixtures/rgb-profile.pcnd"
    ))
    .unwrap()
    .source_metadata;
    assert!(doc.source_metadata.is_some());
    pcnd::write_compact(&doc).unwrap()
}
#[derive(Clone, Copy)]
enum Format {
    Ico,
    Cur,
    Icns,
    Dcx,
    Dds,
    Msp,
}
impl Format {
    fn suffix(self) -> &'static str {
        match self {
            Self::Ico => "ico",
            Self::Cur => "cur",
            Self::Icns => "icns",
            Self::Dcx => "dcx",
            Self::Dds => "dds",
            Self::Msp => "msp",
        }
    }
    fn cli(self) -> Vec<&'static str> {
        match self {
            Self::Ico => vec![
                "export-icons",
                "--entry",
                "2:png",
                "--resampling",
                "nearest",
            ],
            Self::Cur => vec![
                "export-icons",
                "--entry",
                "2:bitmap:1:0",
                "--resampling",
                "nearest",
            ],
            Self::Icns => vec!["export-icns", "--type", "ic11", "--resampling", "nearest"],
            Self::Dcx => vec!["export-dcx", "--pcx-mode", "rgb24"],
            Self::Dds => vec![
                "render",
                "--dds-encoding",
                "rgba8",
                "--dds-transfer",
                "srgb",
            ],
            Self::Msp => vec!["render", "--msp-version", "v2"],
        }
    }
    fn tool(self) -> &'static str {
        match self {
            Self::Ico | Self::Cur => "export_icons",
            Self::Icns => "export_icns",
            Self::Dcx => "export_dcx",
            Self::Dds | Self::Msp => "export_image",
        }
    }
    fn losses(self) -> Vec<&'static str> {
        if matches!(self, Self::Dcx | Self::Dds | Self::Msp) {
            paintcraft_formats::export::KNOWN_LOSSES.to_vec()
        } else {
            icons::KNOWN_EXPORT_LOSSES.to_vec()
        }
    }
    fn args(self, path: &Path) -> Value {
        match self {
            Self::Ico => {
                json!({"path":path,"entries":[{"width":2,"height":2,"encoding":"png"}],"resampling":"nearest"})
            }
            Self::Cur => {
                json!({"path":path,"entries":[{"width":2,"height":2,"encoding":"bitmap","hotspot":[1,0]}],"resampling":"nearest"})
            }
            Self::Icns => json!({"path":path,"types":["ic11"],"resampling":"nearest"}),
            Self::Dcx => json!({"path":path,"pcx_mode":"rgb24"}),
            Self::Dds => json!({"path":path,"dds":{"encoding":"rgba8","transfer":"srgb"}}),
            Self::Msp => json!({"path":path,"msp_version":"v2"}),
        }
    }
    fn pixels(self, bytes: &[u8]) {
        let (w, h, rgba) = match self {
            Self::Ico | Self::Cur => {
                let d = icons::decode_entry(bytes, 0, 4096).unwrap();
                (d.entry.width, d.entry.height, d.rgba)
            }
            Self::Icns => {
                let d = icns::decode_entry(bytes, 0, 4096, &|| Ok(())).unwrap();
                (d.entry.width, d.entry.height, d.rgba)
            }
            Self::Dcx => {
                let d = dcx::decode_page(bytes, 0, 4096).unwrap();
                (d.width, d.height, d.rgba)
            }
            Self::Dds => {
                let d = dds::decode_with_pixel_limit(bytes, 4096).unwrap();
                (d.info.width, d.info.height, d.rgba)
            }
            Self::Msp => {
                let d = msp::decode(bytes, &|| Ok(())).unwrap();
                (
                    d.header().width(),
                    d.header().height(),
                    d.rgba(&|| Ok(())).unwrap(),
                )
            }
        };
        let size = if matches!(self, Self::Icns) { 32 } else { 2 };
        assert_eq!((w, h), (size, size));
        let expected: Vec<u8> = (0..h)
            .flat_map(|y| {
                (0..w).flat_map(move |x| {
                    let i = ((y * 2 / h) * 2 + x * 2 / w) as usize * 4;
                    RGBA[i..i + 4].to_vec()
                })
            })
            .collect();
        assert_eq!(rgba, expected, "independently authored opaque checkerboard");
    }
}
#[test]
fn actual_cli_and_stdio_assets_require_copy_intent_and_preserve_source_and_session() {
    let folder = folder();
    let source = folder.0.join("source.pcnd");
    let original = project();
    fs::write(&source, &original).unwrap();
    let mut mcp = mcp_client::Client::at(Path::new(env!("CARGO_BIN_EXE_paintcraft-cli")));
    mcp.ok("open_project", json!({"path":source}));
    let state = mcp.ok("inspect_document", json!({}));
    for format in [
        Format::Ico,
        Format::Cur,
        Format::Icns,
        Format::Dcx,
        Format::Dds,
        Format::Msp,
    ] {
        for (name, intent, success) in [
            ("default", None, false),
            ("refuse", Some("refuse"), false),
            ("omit", Some("omit"), true),
        ] {
            let cli = folder.0.join(format!("cli-{name}.{}", format.suffix()));
            let remote = folder.0.join(format!("mcp-{name}.{}", format.suffix()));
            fs::write(&cli, b"existing").unwrap();
            fs::write(&remote, b"existing").unwrap();
            let mut cmd = Command::new(env!("CARGO_BIN_EXE_paintcraft-cli"));
            let args = format.cli();
            cmd.arg(args[0])
                .arg(&source)
                .args(&args[1..])
                .arg("-o")
                .arg(&cli)
                .args(["--overwrite", "--json"]);
            let losses: Vec<_> = format
                .losses()
                .into_iter()
                .filter(|loss| *loss != paintcraft_formats::export::metadata_policy::OMITTED_REASON)
                .collect();
            cmd.arg("--allow-loss").arg(losses.join(","));
            if let Some(intent) = intent {
                cmd.args(["--source-metadata", intent]);
            }
            let result = cmd.output().unwrap();
            assert_eq!(
                result.status.success(),
                success,
                "{} {}",
                String::from_utf8_lossy(&result.stdout),
                String::from_utf8_lossy(&result.stderr)
            );
            if !success {
                assert!(
                    String::from_utf8_lossy(&result.stdout).contains("explicit source metadata"),
                    "{} {}",
                    String::from_utf8_lossy(&result.stdout),
                    String::from_utf8_lossy(&result.stderr)
                );
            }
            let mut request = format.args(&remote);
            request["overwrite"] = true.into();
            request["allow_losses"] = json!(losses);
            if let Some(intent) = intent {
                request["source_metadata"] = intent.into();
            }
            let result = mcp.call(format.tool(), request);
            assert_eq!(result["isError"] == true, !success, "{result}");
            if success {
                let bytes = fs::read(&cli).unwrap();
                assert_eq!(bytes, fs::read(&remote).unwrap());
                format.pixels(&bytes);
                assert!(
                    result
                        .to_string()
                        .contains(paintcraft_formats::export::metadata_policy::OMITTED_REASON)
                );
            } else {
                assert_eq!(fs::read(&cli).unwrap(), b"existing");
                assert_eq!(fs::read(&remote).unwrap(), b"existing");
            }
            assert_eq!(fs::read(&source).unwrap(), original);
            assert_eq!(mcp.ok("inspect_document", json!({})), state);
        }
    }
    // Metadata intent acknowledges only metadata, never an actual resize loss.
    let cli = folder.0.join("unacknowledged-resize.icns");
    fs::write(&cli, b"existing").unwrap();
    let result = Command::new(env!("CARGO_BIN_EXE_paintcraft-cli"))
        .arg("export-icns")
        .arg(&source)
        .args([
            "--type",
            "ic11",
            "--source-metadata",
            "omit",
            "--overwrite",
            "--json",
        ])
        .arg("-o")
        .arg(&cli)
        .output()
        .unwrap();
    assert!(!result.status.success());
    assert!(String::from_utf8_lossy(&result.stdout).contains("icon-resized"));
    assert_eq!(fs::read(&cli).unwrap(), b"existing");
    let result=mcp.call("export_icns",json!({"path":cli,"types":["ic11"],"source_metadata":"omit","overwrite":true,"allow_losses":[]}));
    assert_eq!(result["isError"], true, "{result}");
    assert_eq!(fs::read(&cli).unwrap(), b"existing");
    assert_eq!(mcp.ok("inspect_document", json!({})), state);
    assert_eq!(fs::read(&source).unwrap(), original);
}
