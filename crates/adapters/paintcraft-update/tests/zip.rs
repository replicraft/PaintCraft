//! Hostile archives never write outside the target and never expand without bound.

use std::io::Write;

use paintcraft_update::Error;
use paintcraft_update::zipx::{Limits, extract};

mod support;
use support::zips::{app_zip, file_options, zip_of, zip_with_symlink};
use support::{tempdir, write};

fn run(bytes: &[u8], limits: &Limits) -> (tempfile::TempDir, Result<std::path::PathBuf, Error>) {
    let dir = tempdir();
    let archive = write(dir.path().join("in.zip"), bytes);
    let out = dir.path().join("out");
    std::fs::create_dir(&out).unwrap();
    let result = extract(&archive, &out, "PaintCraft.app", limits);
    (dir, result)
}

fn outside_is_empty(dir: &tempfile::TempDir) {
    let mut names: Vec<_> = std::fs::read_dir(dir.path())
        .unwrap()
        .flatten()
        .map(|e| e.file_name())
        .collect();
    names.sort();
    assert_eq!(
        names,
        ["in.zip", "out"],
        "nothing may appear next to the target"
    );
}

#[test]
fn a_good_bundle_extracts_with_modes() {
    let (dir, result) = run(
        &app_zip("0.1.1", "io.github.replicraft.PaintCraft"),
        &Limits::APP,
    );
    let app = result.unwrap();
    assert!(app.join("Contents/Info.plist").is_file());
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let mode = |p: &str| std::fs::metadata(app.join(p)).unwrap().permissions().mode() & 0o777;
        assert_eq!(mode("Contents/MacOS/paintcraft-launcher"), 0o755);
        assert_eq!(mode("Contents/Info.plist"), 0o644);
    }
    outside_is_empty(&dir);
}

#[test]
fn path_tricks_are_refused() {
    for name in [
        "../evil",
        "PaintCraft.app/../../evil",
        "/etc/passwd",
        "PaintCraft.app/../x",
        "PaintCraft.app/./x",
        "PaintCraft.app//x",
        "Other.app/x",
        "evil",
        "PaintCraft.app\\..\\x",
        "C:/evil",
        "PaintCraft.app/a:b",
        "PaintCraft.app/x\0y",
        "PaintCraft.app/con.",
        "PaintCraft.app/trail ",
    ] {
        let (dir, result) = run(&zip_of(&[(name, b"x", 0o644)]), &Limits::APP);
        assert!(
            matches!(result, Err(Error::UnsafeArchive(_))),
            "{name:?}: {result:?}"
        );
        outside_is_empty(&dir);
    }
}

#[test]
fn links_and_devices_are_refused() {
    let (dir, result) = run(&zip_with_symlink(), &Limits::APP);
    assert!(matches!(result, Err(Error::UnsafeArchive(_))), "{result:?}");
    outside_is_empty(&dir);
    // Character device, FIFO and socket: the file type bits patched into the central directory.
    for kind in [0o020644u32, 0o010644, 0o140644] {
        let mut bytes = zip_of(&[("PaintCraft.app/dev", b"", 0o644)]);
        let at = bytes.windows(4).position(|w| w == b"PK\x01\x02").unwrap();
        bytes[at + 38..at + 42].copy_from_slice(&(kind << 16).to_le_bytes());
        let (_, result) = run(&bytes, &Limits::APP);
        assert!(
            matches!(result, Err(Error::UnsafeArchive(_))),
            "{kind:o}: {result:?}"
        );
    }
}

#[test]
fn duplicates_are_refused() {
    // The writer refuses duplicates, so rename the second entry in the raw bytes (same length).
    let mut bytes = zip_of(&[
        ("PaintCraft.app/a1", b"1", 0o644),
        ("PaintCraft.app/a2", b"2", 0o644),
    ]);
    let original = b"PaintCraft.app/a2";
    let duplicate = b"PaintCraft.app/a1";
    let mut at = 0;
    let mut patched = 0;
    while let Some(i) = bytes[at..]
        .windows(original.len())
        .position(|w| w == original)
    {
        bytes[at + i..at + i + original.len()].copy_from_slice(duplicate);
        at += i + original.len();
        patched += 1;
    }
    assert_eq!(
        patched, 2,
        "local and central directory names must both change"
    );
    // The zip crate itself collapses duplicate names (last wins) while reading the directory, so
    // either our check fires or exactly one file results; never two writes to one path.
    let (_dir, result) = run(&bytes, &Limits::APP);
    match result {
        Err(Error::UnsafeArchive(_)) => {}
        Ok(app) => assert_eq!(std::fs::read_dir(app).unwrap().count(), 1),
        other => panic!("{other:?}"),
    }
}

#[test]
fn setuid_and_group_write_bits_are_dropped() {
    let (_dir, result) = run(
        &zip_of(&[("PaintCraft.app/tool", b"x", 0o104777)]),
        &Limits::APP,
    );
    let app = result.unwrap();
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        assert_eq!(
            std::fs::metadata(app.join("tool"))
                .unwrap()
                .permissions()
                .mode()
                & 0o7777,
            0o755
        );
    }
    let _ = app;
}

#[test]
fn too_many_entries_are_refused() {
    let entries: Vec<(String, &[u8], u32)> = (0..20)
        .map(|i| (format!("PaintCraft.app/f{i}"), &b"x"[..], 0o644))
        .collect();
    let refs: Vec<(&str, &[u8], u32)> = entries
        .iter()
        .map(|(n, b, m)| (n.as_str(), *b, *m))
        .collect();
    let limits = Limits {
        max_entries: 10,
        ..Limits::APP
    };
    let (_, result) = run(&zip_of(&refs), &limits);
    assert!(matches!(result, Err(Error::UnsafeArchive(_))));
}

#[test]
fn a_zip_bomb_is_stopped_by_ratio_and_by_total_size() {
    // 8 MiB of zeros deflates to about 8 KiB: ratio near 1000.
    let zeros = vec![0u8; 8 * 1024 * 1024];
    let bomb = zip_of(&[("PaintCraft.app/zeros", &zeros, 0o644)]);
    assert!(bomb.len() < 100 * 1024);
    let (_, result) = run(&bomb, &Limits::APP);
    assert!(matches!(result, Err(Error::UnsafeArchive(m)) if m.contains("compression")));
    // With the ratio check out of the way the total cap still stops it.
    let limits = Limits {
        max_ratio: u64::MAX,
        max_total: 1024 * 1024,
        ..Limits::APP
    };
    let (dir, result) = run(&bomb, &limits);
    assert!(matches!(result, Err(Error::UnsafeArchive(m)) if m.contains("beyond")));
    let written = std::fs::metadata(dir.path().join("out/PaintCraft.app/zeros"))
        .map(|m| m.len())
        .unwrap_or(0);
    assert!(written <= 1024 * 1024, "stopped early, wrote {written}");
}

#[test]
fn a_header_that_lies_about_the_size_is_caught() {
    // Build a good archive, then patch the uncompressed size in the central directory and local header.
    let mut bytes = zip_of(&[("PaintCraft.app/f", &[7u8; 5000], 0o644)]);
    let size = 5000u32.to_le_bytes();
    let small = 10u32.to_le_bytes();
    let mut patched = 0;
    for i in 0..bytes.len() - 4 {
        if bytes[i..i + 4] == size && (bytes[i.saturating_sub(4)..i] != [0; 4]) {
            bytes[i..i + 4].copy_from_slice(&small);
            patched += 1;
        }
    }
    assert!(patched > 0);
    let (_, result) = run(&bytes, &Limits::APP);
    assert!(result.is_err());
}

#[test]
fn garbage_and_empty_archives_fail_cleanly() {
    let (_, result) = run(b"not a zip at all", &Limits::APP);
    assert!(matches!(result, Err(Error::Archive(_))));
    let mut writer = zip::ZipWriter::new(std::io::Cursor::new(Vec::new()));
    writer
        .add_directory("PaintCraft.app/", file_options(0o755))
        .unwrap();
    let empty_ok = writer.finish().unwrap().into_inner();
    let (_, result) = run(&empty_ok, &Limits::APP);
    assert!(result.is_ok());
    let mut writer = zip::ZipWriter::new(std::io::Cursor::new(Vec::new()));
    writer
        .start_file("PaintCraft.app/x", file_options(0o644))
        .unwrap();
    writer.write_all(b"x").unwrap();
    let _ = writer;
    let (_, result) = run(
        &zip::ZipWriter::new(std::io::Cursor::new(Vec::new()))
            .finish()
            .unwrap()
            .into_inner(),
        &Limits::APP,
    );
    assert!(result.is_err());
}
