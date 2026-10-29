//! The real command's existing post-staging checkpoint, without timing races.
use super::*;
use clap::Parser;
use std::fs;
struct Folder(PathBuf);
impl Folder {
    fn new() -> Self {
        let p = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("../../docs/validation/stock-source/cli-unit")
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
fn command(args: Vec<String>) -> Reply {
    let cli = crate::Cli::try_parse_from(args).unwrap();
    let crate::Command::Import(options) = cli.command else {
        panic!()
    };
    run(options, true)
}
#[test]
fn actual_stock_import_post_staging_checkpoint_rejects_changed_sources_and_renamed_aliases() {
    for (format, ext, losses) in [
        (ImageFormat::Png, "png", ""),
        (ImageFormat::Bmp, "bmp", "bmp-color-metadata-ignored"),
        (ImageFormat::Jpeg, "jpg", ""),
        (ImageFormat::Gif, "gif", "palette-indices-expanded"),
        (ImageFormat::Tiff, "tiff", "tiff-metadata-ignored"),
    ] {
        for mutation in 0..3 {
            let folder = Folder::new();
            let source = folder.0.join(format!("source.{ext}"));
            let output = folder.0.join("output.pcnd");
            let moved = folder.0.join("moved");
            let bytes = image::encode(format, 2, 1, &[13, 71, 201, 255, 7, 8, 9, 255], 90).unwrap();
            fs::write(&source, &bytes).unwrap();
            fs::write(&output, &bytes).unwrap();
            let mut args = vec![
                "paintcraft-cli".into(),
                "import".into(),
                source.to_str().unwrap().into(),
                "-o".into(),
                output.to_str().unwrap().into(),
                "--overwrite".into(),
            ];
            if !losses.is_empty() {
                args.extend(["--allow-loss".into(), losses.into()]);
            }
            let (src, dst, renamed, original) =
                (source.clone(), output.clone(), moved.clone(), bytes.clone());
            super::super::native_save::inject_at_checkpoint(
                move || {
                    if mutation == 0 {
                        let mut changed = original;
                        *changed.last_mut().unwrap() ^= 1;
                        fs::write(src, changed).unwrap();
                    } else {
                        fs::rename(&src, &renamed).unwrap();
                        fs::write(&src, &original).unwrap();
                        if mutation == 2 {
                            fs::remove_file(&dst).unwrap();
                            fs::hard_link(&renamed, &dst).unwrap();
                        }
                    }
                },
                || {
                    let reply = command(args.clone());
                    assert_ne!(reply.exit, 0, "{ext} mutation={mutation}");
                    let value: serde_json::Value = serde_json::from_slice(&reply.stdout).unwrap();
                    assert_eq!(value["file"]["replacement"], "not-replaced", "{value}");
                    assert!(value["file"]["staged_sha256"].is_string(), "{value}");
                    assert!(value["file"]["acknowledged_state_id"].is_null(), "{value}");
                },
            );
            assert_eq!(fs::read(&output).unwrap(), bytes);
            if mutation == 2 {
                fs::remove_file(&output).unwrap();
                fs::write(&output, &bytes).unwrap();
            }
            if mutation != 0 {
                fs::remove_file(&source).unwrap();
                fs::rename(&moved, &source).unwrap();
            } else {
                fs::write(&source, &bytes).unwrap();
            }
            assert_eq!(command(args).exit, 0);
            assert_eq!(
                pcnd::read(&fs::read(&output).unwrap()).unwrap().canvas,
                (2, 1)
            );
            assert_eq!(fs::read(&source).unwrap(), bytes);
        }
    }
}
