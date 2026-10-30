//! Real CLI/RPC consent boundaries; source rasters and editable PCND stay intact.
use paintcraft_core::types::filters::Descriptor;
use paintcraft_document::{FilterInstance, FilterStack};
use paintcraft_formats::{dcx, dds, icns, icons, image, msp, pcnd};
use serde_json::{Value, json};
use std::{
    fs,
    path::{Path, PathBuf},
    process::Command,
};
#[path = "support/mcp_client.rs"]
mod mcp_client;
const SOURCE: [u8; 16] = [
    0, 0, 0, 255, 255, 255, 255, 255, 255, 255, 255, 255, 0, 0, 0, 255,
];
struct Folder(PathBuf);
impl Folder {
    fn new() -> Self {
        let p = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("../../docs/validation/filter-copy-policies")
            .join(uuid::Uuid::new_v4().to_string());
        fs::create_dir_all(&p).unwrap();
        Self(p)
    }
}
impl Drop for Folder {
    fn drop(&mut self) {
        fs::remove_dir_all(&self.0).unwrap();
    }
}
fn project(enabled: bool) -> Vec<u8> {
    let mut doc =
        paintcraft_document::import_image(2, 2, &SOURCE, "filter-copy-authored-checkerboard")
            .unwrap();
    doc.layers.get_mut(&doc.roots[0]).unwrap().filters =
        FilterStack::new(vec![FilterInstance::new(
            uuid::Uuid::from_u128(1),
            enabled,
            Descriptor::InvertRgbV1,
        )])
        .unwrap();
    doc.state_id = uuid::Uuid::from_u128(if enabled { 700 } else { 701 });
    doc.source_metadata = pcnd::read(include_bytes!(
        "../../../crates/services/paintcraft-formats/tests/pcnd-metadata-fixtures/rgb-profile.pcnd"
    ))
    .unwrap()
    .source_metadata;
    pcnd::write_compact(&doc).unwrap()
}
#[derive(Clone, Copy, Debug)]
enum Format {
    Png,
    Dds,
    Msp,
    Dcx,
    Ico,
    Cur,
    Icns,
}
impl Format {
    fn suffix(self) -> &'static str {
        match self {
            Self::Png => "png",
            Self::Dds => "dds",
            Self::Msp => "msp",
            Self::Dcx => "dcx",
            Self::Ico => "ico",
            Self::Cur => "cur",
            Self::Icns => "icns",
        }
    }
    fn cli(self) -> Vec<&'static str> {
        match self {
            Self::Png => vec!["render"],
            Self::Dds => vec![
                "render",
                "--dds-encoding",
                "rgba8",
                "--dds-transfer",
                "srgb",
            ],
            Self::Msp => vec!["render", "--msp-version", "v2"],
            Self::Dcx => vec!["export-dcx", "--pcx-mode", "rgb24"],
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
                "2:png:1:0",
                "--resampling",
                "nearest",
            ],
            Self::Icns => vec!["export-icns", "--type", "ic11", "--resampling", "nearest"],
        }
    }
    fn tool(self) -> &'static str {
        match self {
            Self::Png | Self::Dds | Self::Msp => "export_image",
            Self::Dcx => "export_dcx",
            Self::Ico | Self::Cur => "export_icons",
            Self::Icns => "export_icns",
        }
    }
    fn args(self, path: &Path) -> Value {
        match self {
            Self::Png => json!({"path":path}),
            Self::Dds => json!({"path":path,"dds":{"encoding":"rgba8","transfer":"srgb"}}),
            Self::Msp => json!({"path":path,"msp_version":"v2"}),
            Self::Dcx => json!({"path":path,"pcx_mode":"rgb24"}),
            Self::Ico => {
                json!({"path":path,"entries":[{"width":2,"height":2,"encoding":"png"}],"resampling":"nearest"})
            }
            Self::Cur => {
                json!({"path":path,"entries":[{"width":2,"height":2,"encoding":"png","hotspot":[1,0]}],"resampling":"nearest"})
            }
            Self::Icns => json!({"path":path,"types":["ic11"],"resampling":"nearest"}),
        }
    }
    fn pixels(self, bytes: &[u8], enabled: bool) {
        let (w, h, rgba) = match self {
            Self::Png => {
                let d = image::decode(bytes).unwrap();
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
            Self::Dcx => {
                let d = dcx::decode_page(bytes, 0, 4096).unwrap();
                (d.width, d.height, d.rgba)
            }
            Self::Ico | Self::Cur => {
                let d = icons::decode_entry(bytes, 0, 4096).unwrap();
                (d.entry.width, d.entry.height, d.rgba)
            }
            Self::Icns => {
                let d = icns::decode_entry(bytes, 0, 4096, &|| Ok(())).unwrap();
                (d.entry.width, d.entry.height, d.rgba)
            }
        };
        let edge = if matches!(self, Self::Icns) { 32 } else { 2 };
        assert_eq!((w, h), (edge, edge));
        let expected: Vec<u8> = (0..h)
            .flat_map(|y| {
                (0..w).flat_map(move |x| {
                    let i = ((y * 2 / h) * 2 + x * 2 / w) as usize * 4;
                    let mut p = SOURCE[i..i + 4].to_vec();
                    if enabled {
                        for c in &mut p[..3] {
                            *c = 255 - *c;
                        }
                    }
                    p
                })
            })
            .collect();
        assert_eq!(
            rgba, expected,
            "independent checkerboard, {self:?}, enabled={enabled}"
        );
    }
    fn losses(self, bake: bool) -> Vec<&'static str> {
        let mut losses = vec![];
        if matches!(self, Self::Icns) {
            losses.push("icon-resized");
        }
        if bake {
            losses.push("filters-baked");
        }
        losses
    }
}
#[test]
fn explicit_cli_and_stdio_bake_consent_is_independent_and_copy_only_for_active_or_disabled_stacks()
{
    let folder = Folder::new();
    for enabled in [true, false] {
        let source = folder.0.join(format!("source-{enabled}.pcnd"));
        let original = project(enabled);
        fs::write(&source, &original).unwrap();
        let mut client = mcp_client::Client::at(Path::new(env!("CARGO_BIN_EXE_paintcraft-cli")));
        client.ok("open_project", json!({"path":source}));
        let before = client.ok("inspect_document", json!({}));
        for format in [
            Format::Png,
            Format::Dds,
            Format::Msp,
            Format::Dcx,
            Format::Ico,
            Format::Cur,
            Format::Icns,
        ] {
            for (name, omit, bake, success) in [
                ("only-metadata", true, false, false),
                ("only-filters", false, true, false),
                ("both", true, true, true),
            ] {
                let cli = folder
                    .0
                    .join(format!("cli-{enabled}-{name}.{}", format.suffix()));
                let remote = folder
                    .0
                    .join(format!("mcp-{enabled}-{name}.{}", format.suffix()));
                for p in [&cli, &remote] {
                    fs::write(p, b"existing destination").unwrap();
                }
                let losses = format.losses(bake);
                let mut command = Command::new(env!("CARGO_BIN_EXE_paintcraft-cli"));
                command
                    .args(format.cli())
                    .arg(&source)
                    .arg("-o")
                    .arg(&cli)
                    .args(["--json", "--overwrite"]);
                if omit {
                    command.args(["--source-metadata", "omit"]);
                }
                if !losses.is_empty() {
                    command.arg("--allow-loss").arg(losses.join(","));
                }
                let output = command.output().unwrap();
                assert_eq!(
                    output.status.success(),
                    success,
                    "{format:?}/{name}:{} {}",
                    String::from_utf8_lossy(&output.stdout),
                    String::from_utf8_lossy(&output.stderr)
                );
                let mut args = format.args(&remote);
                args["overwrite"] = true.into();
                args["allow_losses"] = json!(losses);
                if omit {
                    args["source_metadata"] = "omit".into();
                }
                let result = client.call(format.tool(), args);
                assert_eq!(
                    result["isError"] != true,
                    success,
                    "{format:?}/{name}: {result}"
                );
                if success {
                    assert!(String::from_utf8_lossy(&output.stdout).contains("filters-baked"));
                    assert!(result.to_string().contains("filters-baked"));
                    format.pixels(&fs::read(&cli).unwrap(), enabled);
                    format.pixels(&fs::read(&remote).unwrap(), enabled);
                } else {
                    for p in [&cli, &remote] {
                        assert_eq!(fs::read(p).unwrap(), b"existing destination");
                    }
                }
                assert_eq!(client.ok("inspect_document", json!({})), before);
                assert_eq!(fs::read(&source).unwrap(), original);
            }
        }
        let alias = folder.0.join(format!("alias-{enabled}.png"));
        fs::hard_link(&source, &alias).unwrap();
        assert_eq!(client.call("export_image",json!({"path":alias,"overwrite":true,"source_metadata":"omit","allow_losses":["filters-baked"]}))["isError"],true);
        assert_eq!(fs::read(&source).unwrap(), original);
        assert_eq!(client.ok("inspect_document", json!({})), before);
    }
}
