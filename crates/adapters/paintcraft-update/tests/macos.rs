//! The macOS bundle swap on fake bundles in a temp directory. `codesign` is injected; nothing
//! here signs, runs or opens anything.
#![cfg(unix)]

use std::path::Path;
use std::sync::atomic::{AtomicUsize, Ordering};

use paintcraft_update::Error;
use paintcraft_update::apply::macos::{self as macos, cleanup_stale, preflight, prepare, swap};

mod support;
use support::zips::{app_zip, zip_of};
use support::{tempdir, write};

fn version() -> semver::Version {
    paintcraft_update::version::parse("0.1.1").unwrap()
}

fn accept(_: &Path) -> Result<(), Error> {
    Ok(())
}

fn fake_bundle(parent: &Path, marker: &str) -> std::path::PathBuf {
    let bundle = parent.join("PaintCraft.app");
    std::fs::create_dir_all(bundle.join("Contents/MacOS")).unwrap();
    write(bundle.join("Contents/Info.plist"), marker.as_bytes());
    bundle
}

fn marker(bundle: &Path) -> String {
    std::fs::read_to_string(bundle.join("Contents/Info.plist")).unwrap()
}

fn staged(zip: &[u8]) -> (tempfile::TempDir, std::path::PathBuf) {
    let dir = tempdir();
    let path = write(dir.path().join("update.zip"), zip);
    (dir, path)
}

#[test]
fn a_valid_bundle_is_prepared_and_codesign_is_asked_once() {
    let (dir, zip) = staged(&app_zip("0.1.1", macos::BUNDLE_ID));
    let calls = AtomicUsize::new(0);
    let app = prepare(&zip, dir.path(), &version(), &|path| {
        assert!(path.ends_with("PaintCraft.app"));
        calls.fetch_add(1, Ordering::SeqCst);
        Ok(())
    })
    .unwrap();
    assert_eq!(calls.load(Ordering::SeqCst), 1);
    assert!(app.join("Contents/MacOS/paintcraft-launcher").is_file());
}

#[test]
fn a_failing_codesign_check_fails_the_update() {
    let (dir, zip) = staged(&app_zip("0.1.1", macos::BUNDLE_ID));
    let result = prepare(&zip, dir.path(), &version(), &|_| {
        Err(Error::CodesignFailed("a sealed resource is missing".into()))
    });
    assert!(matches!(result, Err(Error::CodesignFailed(_))));
}

#[test]
fn the_bundle_must_be_the_expected_one() {
    // Wrong identifier, wrong version, missing executable, non-executable binary, no plist.
    let cases: Vec<(&str, Vec<u8>)> = vec![
        ("identifier", app_zip("0.1.1", "com.evil.app")),
        ("version", app_zip("0.1.2", macos::BUNDLE_ID)),
        ("version", app_zip("0.1.0", macos::BUNDLE_ID)),
        (
            "executable",
            zip_of(&[(
                "PaintCraft.app/Contents/Info.plist",
                support::zips::INFO_PLIST
                    .replace("NUMERIC", "0.1.1")
                    .as_bytes(),
                0o644,
            )]),
        ),
        ("not executable", {
            let plist = support::zips::INFO_PLIST.replace("NUMERIC", "0.1.1");
            zip_of(&[
                (
                    "PaintCraft.app/Contents/Info.plist",
                    plist.as_bytes(),
                    0o644,
                ),
                (
                    "PaintCraft.app/Contents/MacOS/paintcraft-launcher",
                    b"x",
                    0o644,
                ),
            ])
        }),
        (
            "plist",
            zip_of(&[(
                "PaintCraft.app/Contents/MacOS/paintcraft-launcher",
                b"x",
                0o755,
            )]),
        ),
        (
            "binary plist",
            zip_of(&[
                ("PaintCraft.app/Contents/Info.plist", b"bplist00\x00", 0o644),
                (
                    "PaintCraft.app/Contents/MacOS/paintcraft-launcher",
                    b"x",
                    0o755,
                ),
            ]),
        ),
    ];
    for (what, bytes) in cases {
        let (dir, zip) = staged(&bytes);
        let result = prepare(&zip, dir.path(), &version(), &accept);
        assert!(
            matches!(result, Err(Error::BundleInvalid(_))),
            "{what}: {result:?}"
        );
    }
}

#[test]
fn prerelease_versions_match_numeric_bundle_and_exact_helper_version() {
    let (dir, zip) = staged(&support::zips::app_zip_release(
        "1.2.3",
        macos::BUNDLE_ID,
        "1.2.3-rc.1",
    ));
    let version = paintcraft_update::version::parse("1.2.3-rc.1").unwrap();
    assert!(prepare(&zip, dir.path(), &version, &accept).is_ok());
}

#[test]
fn swap_puts_the_new_app_in_place_and_keeps_the_old_one_aside() {
    let dir = tempdir();
    let bundle = fake_bundle(dir.path(), "old");
    let staging = tempdir();
    let new_app = fake_bundle(staging.path(), "new");
    let old = swap(&bundle, &new_app).unwrap();
    assert_eq!(marker(&bundle), "new");
    assert_eq!(marker(&old), "old");
    let name = old.file_name().unwrap().to_string_lossy().into_owned();
    assert!(
        name.starts_with(".PaintCraft.app.old-") && name.len() == ".PaintCraft.app.old-".len() + 32,
        "{name}"
    );
    assert_eq!(old.parent().unwrap(), dir.path());
    assert!(!new_app.exists());
    // The next start removes the leftover, and only that.
    let other = dir.path().join(".PaintCraft.app.old-notours");
    std::fs::create_dir(&other).unwrap();
    cleanup_stale(&bundle);
    assert!(!old.exists() && other.exists() && bundle.exists());
}

#[test]
fn swap_rolls_back_when_the_new_app_cannot_be_moved() {
    let dir = tempdir();
    let bundle = fake_bundle(dir.path(), "old");
    let missing = dir.path().join("nowhere/PaintCraft.app");
    let result = swap(&bundle, &missing);
    assert!(matches!(result, Err(Error::Io { .. })));
    assert_eq!(marker(&bundle), "old", "the old app is back");
    let names: Vec<_> = std::fs::read_dir(dir.path())
        .unwrap()
        .flatten()
        .map(|e| e.file_name())
        .collect();
    assert_eq!(names.len(), 1, "no aside copy left: {names:?}");
}

#[test]
fn translocated_and_read_only_locations_are_refused() {
    let result = preflight(Path::new(
        "/private/var/folders/ab/xyz/AppTranslocation/1234-ABCD/d/PaintCraft.app",
    ));
    assert!(matches!(result, Err(Error::AppTranslocated)));
    let dir = tempdir();
    let bundle = fake_bundle(dir.path(), "old");
    assert!(preflight(&bundle).is_ok());
    // A directory the user cannot write into (root-owned location stands in for a read-only disk image).
    let result = preflight(Path::new("/System/Applications/PaintCraft.app"));
    assert!(
        matches!(
            result,
            Err(Error::ReadOnlyVolume(_)) | Err(Error::ParentNotWritable(_))
        ),
        "{result:?}"
    );
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(dir.path(), std::fs::Permissions::from_mode(0o555)).unwrap();
        let result = preflight(&bundle);
        std::fs::set_permissions(dir.path(), std::fs::Permissions::from_mode(0o755)).unwrap();
        if rustix::process::getuid().is_root() {
            assert!(result.is_ok());
        } else {
            assert!(
                matches!(result, Err(Error::ParentNotWritable(_))),
                "{result:?}"
            );
        }
    }
}

#[test]
fn missing_or_mismatched_nested_editor_and_helper_are_refused_before_codesign() {
    use std::io::{Cursor, Read, Write};
    use support::zips::file_options;
    let input = app_zip("0.1.1", macos::BUNDLE_ID);
    for variant in 0..6 {
        let mut reader = zip::ZipArchive::new(Cursor::new(&input)).unwrap();
        let mut output = zip::ZipWriter::new(Cursor::new(Vec::new()));
        for index in 0..reader.len() {
            let mut entry = reader.by_index(index).unwrap();
            let name = entry.name().unwrap().into_owned();
            let mode = entry.unix_mode().unwrap_or(0o644);
            let mut bytes = Vec::new();
            entry.read_to_end(&mut bytes).unwrap();
            let editor_info = name.ends_with("PaintCraft Editor.app/Contents/Info.plist");
            let helper = name.ends_with("/MacOS/paintcraft-codec-worker");
            let manifest = name.ends_with("/Resources/paintcraft-codec-worker.json");
            if variant == 0 && editor_info || variant == 1 && helper {
                continue;
            }
            if variant == 2 && editor_info {
                bytes = String::from_utf8(bytes)
                    .unwrap()
                    .replace("PaintCraft.editor", "WrongEditor")
                    .into_bytes()
            }
            if variant == 3 && manifest {
                bytes = String::from_utf8(bytes)
                    .unwrap()
                    .replace("0.1.1", "0.1.0")
                    .into_bytes()
            }
            if variant == 4 && helper {
                bytes = b"different helper".to_vec()
            }
            if variant == 5 && manifest {
                bytes = String::from_utf8(bytes)
                    .unwrap()
                    .replace("apple-darwin", "unknown-linux-gnu")
                    .into_bytes()
            }
            if entry.is_dir() {
                output.add_directory(name, file_options(mode)).unwrap();
            } else {
                output.start_file(name, file_options(mode)).unwrap();
                output.write_all(&bytes).unwrap();
            }
        }
        let bytes = output.finish().unwrap().into_inner();
        let (dir, path) = staged(&bytes);
        let calls = AtomicUsize::new(0);
        let result = prepare(&path, dir.path(), &version(), &|_| {
            calls.fetch_add(1, Ordering::SeqCst);
            Ok(())
        });
        assert!(
            matches!(result, Err(Error::BundleInvalid(_))),
            "variant {variant}: {result:?}"
        );
        assert_eq!(calls.load(Ordering::SeqCst), 0, "variant {variant}");
    }
}
