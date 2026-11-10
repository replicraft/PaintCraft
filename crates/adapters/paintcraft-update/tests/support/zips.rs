//! Builders for update archives, honest and hostile.

use std::io::{Cursor, Write};

use zip::write::SimpleFileOptions;

pub const INFO_PLIST: &str = r#"<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0">
<dict>
	<key>CFBundleExecutable</key>
	<string>paintcraft-launcher</string>
	<key>CFBundleIdentifier</key>
	<string>io.github.replicraft.PaintCraft</string>
	<key>CFBundleShortVersionString</key>
	<string>NUMERIC</string>
</dict>
</plist>
"#;

pub fn file_options(mode: u32) -> SimpleFileOptions {
    SimpleFileOptions::default()
        .compression_method(zip::CompressionMethod::Deflated)
        .unix_permissions(mode)
}

/// A well-formed `PaintCraft.app` update archive for `numeric` (major.minor.patch).
pub fn app_zip(numeric: &str, bundle_id: &str) -> Vec<u8> {
    app_zip_release(numeric, bundle_id, numeric)
}
pub fn app_zip_release(numeric: &str, bundle_id: &str, release: &str) -> Vec<u8> {
    let mut writer = zip::ZipWriter::new(Cursor::new(Vec::new()));
    writer
        .add_directory("PaintCraft.app/", file_options(0o755))
        .unwrap();
    writer
        .add_directory("PaintCraft.app/Contents/", file_options(0o755))
        .unwrap();
    writer
        .add_directory("PaintCraft.app/Contents/MacOS/", file_options(0o755))
        .unwrap();
    let plist = INFO_PLIST
        .replace("NUMERIC", numeric)
        .replace("io.github.replicraft.PaintCraft", bundle_id);
    writer
        .start_file("PaintCraft.app/Contents/Info.plist", file_options(0o644))
        .unwrap();
    writer.write_all(plist.as_bytes()).unwrap();
    writer
        .start_file(
            "PaintCraft.app/Contents/MacOS/paintcraft-launcher",
            file_options(0o755),
        )
        .unwrap();
    writer
        .write_all(format!("#!/bin/sh\necho new {numeric}\n").as_bytes())
        .unwrap();
    let editor = "PaintCraft.app/Contents/Helpers/PaintCraft Editor.app/Contents";
    let editor_info = INFO_PLIST
        .replace("NUMERIC", numeric)
        .replace(
            "io.github.replicraft.PaintCraft",
            "io.github.replicraft.PaintCraft.editor",
        )
        .replace("paintcraft-launcher", "paintcraft");
    let helper = b"test helper";
    use sha2::{Digest, Sha256};
    let hash: String = Sha256::digest(helper)
        .iter()
        .map(|b| format!("{b:02x}"))
        .collect();
    let target = if cfg!(target_arch = "aarch64") {
        "aarch64-apple-darwin"
    } else {
        "x86_64-apple-darwin"
    };
    let manifest=serde_json::json!({"application_version":release,"target":target,"executable":"paintcraft-codec-worker","sha256":hash}).to_string();
    for (name, bytes, mode) in [
        (
            format!("{editor}/Info.plist"),
            editor_info.as_bytes(),
            0o644,
        ),
        (
            format!("{editor}/MacOS/paintcraft"),
            b"engine".as_slice(),
            0o755,
        ),
        (
            format!("{editor}/MacOS/paintcraft-codec-worker"),
            helper.as_slice(),
            0o755,
        ),
        (
            format!("{editor}/Resources/paintcraft-codec-worker.json"),
            manifest.as_bytes(),
            0o644,
        ),
    ] {
        writer.start_file(name, file_options(mode)).unwrap();
        writer.write_all(bytes).unwrap();
    }
    writer.finish().unwrap().into_inner()
}

/// An archive of `(name, body, mode)` entries, in order, with raw names.
pub fn zip_of(entries: &[(&str, &[u8], u32)]) -> Vec<u8> {
    let mut writer = zip::ZipWriter::new(Cursor::new(Vec::new()));
    for (name, body, mode) in entries {
        writer.start_file(*name, file_options(*mode)).unwrap();
        writer.write_all(body).unwrap();
    }
    writer.finish().unwrap().into_inner()
}

pub fn zip_with_symlink() -> Vec<u8> {
    let mut writer = zip::ZipWriter::new(Cursor::new(Vec::new()));
    writer
        .start_file("PaintCraft.app/a", file_options(0o644))
        .unwrap();
    writer.write_all(b"a").unwrap();
    writer
        .add_symlink("PaintCraft.app/link", "/etc/passwd", file_options(0o777))
        .unwrap();
    writer.finish().unwrap().into_inner()
}
