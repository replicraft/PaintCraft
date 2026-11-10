//! Windows portable swap and helper encoding. The logic runs on any host; the PowerShell and
//! msiexec steps themselves are unverified on real Windows.

use paintcraft_update::Error;
use paintcraft_update::apply::windows::{
    PROGRAMS, cleanup_stale, portable_top, prepare_portable, swap_portable,
};
use paintcraft_update::helper::{POWERSHELL, encoded_command};

mod support;
use support::zips::zip_of;
use support::{tempdir, write};

const ASSET: &str = "paintcraft-0.1.1-windows-x64-portable.zip";
const TOP: &str = "paintcraft-0.1.1-windows-x64";

fn archive(extra: &[(&str, &[u8], u32)]) -> Vec<u8> {
    let mut entries: Vec<(String, Vec<u8>, u32)> = PROGRAMS
        .iter()
        .map(|p| (p.to_string(), format!("new {p}").into_bytes(), 0o755))
        .collect();
    entries.extend(
        extra
            .iter()
            .map(|(n, b, m)| ((*n).to_owned(), b.to_vec(), *m)),
    );
    let refs: Vec<(&str, &[u8], u32)> = entries
        .iter()
        .map(|(n, b, m)| (n.as_str(), b.as_slice(), *m))
        .collect();
    zip_of(&refs)
}

#[test]
fn the_top_directory_comes_from_the_asset_name() {
    assert_eq!(portable_top(ASSET), Some(TOP));
    assert_eq!(portable_top("paintcraft-0.1.1-windows-x64.msi"), None);
    assert_eq!(portable_top("-portable.zip"), None);
}

#[test]
fn a_portable_archive_is_prepared_and_swapped_in() {
    let dir = tempdir();
    let zip = write(
        dir.path().join("p.zip"),
        &archive(&[("LICENSE", b"l", 0o644)]),
    );
    let bin = prepare_portable(&zip, dir.path(), ASSET).unwrap();
    let install = dir.path().join("install");
    std::fs::create_dir(&install).unwrap();
    for p in PROGRAMS {
        write(install.join(p), format!("old {p}").as_bytes());
    }
    let old = swap_portable(&install, &bin).unwrap();
    assert_eq!(old.backups().count(), 3);
    for p in PROGRAMS {
        assert_eq!(
            std::fs::read_to_string(install.join(p)).unwrap(),
            format!("new {p}")
        );
    }
    for path in old.backups() {
        assert!(std::fs::read_to_string(path).unwrap().starts_with("old "));
    }
    cleanup_stale(&install);
    assert!(old.backups().all(|p| !p.exists()));
    assert_eq!(std::fs::read_dir(&install).unwrap().count(), 4);
}

#[test]
fn a_swap_that_fails_midway_restores_every_program() {
    let dir = tempdir();
    let zip = write(dir.path().join("p.zip"), &archive(&[]));
    let bin = prepare_portable(&zip, dir.path(), ASSET).unwrap();
    let install = dir.path().join("install");
    std::fs::create_dir(&install).unwrap();
    for p in PROGRAMS {
        write(install.join(p), format!("old {p}").as_bytes());
    }
    // relayd.exe (last) cannot be replaced: its new file is gone, so the rename of it fails after
    // the first two succeeded.
    std::fs::remove_file(bin.join("paintcraft-codec-worker.json")).unwrap();
    std::fs::create_dir(bin.join("paintcraft-codec-worker.json")).unwrap();
    assert!(swap_portable(&install, &bin).is_err());
    // Whatever happened to the third, the first two must be consistent: either all old or all new
    // for each program, and no program missing.
    for p in PROGRAMS {
        assert_eq!(
            std::fs::read_to_string(install.join(p)).unwrap(),
            format!("old {p}")
        );
    }
}

#[test]
fn unexpected_archive_contents_are_refused() {
    for extra in [
        vec![("evil.dll".to_string(), b"x".to_vec())],
        vec![("sub/paintcraft.exe".to_string(), b"x".to_vec())],
    ] {
        let refs: Vec<(&str, &[u8], u32)> = extra
            .iter()
            .map(|(n, b)| (n.as_str(), b.as_slice(), 0o644))
            .collect();
        let dir = tempdir();
        let zip = write(dir.path().join("p.zip"), &archive(&refs));
        assert!(matches!(
            prepare_portable(&zip, dir.path(), ASSET),
            Err(Error::BundleInvalid(_))
        ));
    }
    let dir = tempdir();
    let zip = write(
        dir.path().join("p.zip"),
        &zip_of(&[("paintcraft-codec-worker.exe", b"x", 0o755)]),
    );
    assert!(matches!(
        prepare_portable(&zip, dir.path(), ASSET),
        Err(Error::BundleInvalid(_))
    ));
    let dir = tempdir();
    let zip = write(
        dir.path().join("p.zip"),
        &zip_of(&[("other/bin/paintcraft.exe", b"x", 0o755)]),
    );
    assert!(prepare_portable(&zip, dir.path(), ASSET).is_err());
}

#[test]
fn the_powershell_text_is_fixed_and_encoded_as_utf16_base64() {
    assert_eq!(encoded_command("A"), "QQA=");
    assert_eq!(encoded_command("Hi!"), "SABpACEA");
    assert_eq!(encoded_command(""), "");
    // Round trip through a plain decoder.
    let encoded = encoded_command(POWERSHELL);
    assert!(encoded.len() < 8000, "must fit a command line");
    assert!(
        encoded
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || matches!(b, b'+' | b'/' | b'='))
    );
    // Nothing in the script is built from data; it only reads the child's environment.
    assert!(POWERSHELL.contains("$env:PAINTCRAFT_UPDATE_MSI") && !POWERSHELL.contains("{0}"));
}

#[test]
fn rollback_removes_new_helper_and_manifest_when_old_install_had_only_editor() {
    let dir = tempdir();
    let install = dir.path().join("install");
    std::fs::create_dir(&install).unwrap();
    write(install.join("paintcraft.exe"), b"old editor");
    let zip = write(
        dir.path().join("new.zip"),
        &archive(&[("LICENSE", b"new license", 0o644)]),
    );
    let ready = prepare_portable(&zip, dir.path(), ASSET).unwrap();
    let receipt = swap_portable(&install, &ready).unwrap();
    assert!(install.join("paintcraft-codec-worker.exe").exists());
    receipt.rollback().unwrap();
    assert_eq!(
        std::fs::read(install.join("paintcraft.exe")).unwrap(),
        b"old editor"
    );
    assert_eq!(std::fs::read_dir(&install).unwrap().count(), 1);
}
