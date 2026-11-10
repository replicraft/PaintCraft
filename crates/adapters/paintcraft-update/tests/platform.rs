use std::ffi::OsString;
use std::path::PathBuf;

use paintcraft_update::{Arch, DetectInput, InstallKind, ManualReason, Os, Platform};

mod support;

fn input(os: Os, arch: Arch, exe: &str) -> DetectInput {
    DetectInput {
        os,
        arch,
        exe: Some(PathBuf::from(exe)),
        appimage: None,
        appdir: None,
        program_files: Vec::new(),
        debug_build: false,
    }
}

fn version(text: &str) -> semver::Version {
    paintcraft_update::version::parse(text).unwrap()
}

#[test]
fn macos_bundle_is_found_from_the_executable() {
    let p = Platform::from_input(&input(
        Os::Macos,
        Arch::Arm64,
        "/Applications/PaintCraft.app/Contents/MacOS/paintcraft",
    ));
    assert_eq!(
        p.install,
        InstallKind::MacosBundle {
            bundle: "/Applications/PaintCraft.app".into()
        }
    );
    assert_eq!(
        p.asset_name(&version("0.1.1")).unwrap(),
        "paintcraft-0.1.1-macos-arm64-update.zip"
    );
    let p = Platform::from_input(&input(
        Os::Macos,
        Arch::X64,
        "/Users/a/PaintCraft.app/Contents/MacOS/paintcraft",
    ));
    assert_eq!(
        p.asset_name(&version("1.0.0-rc.1")).unwrap(),
        "paintcraft-1.0.0-rc.1-macos-x64-update.zip"
    );
}

#[test]
fn macos_outside_a_bundle_is_manual() {
    for exe in [
        "/usr/local/bin/paintcraft",
        "/Applications/PaintCraft.app/Contents/paintcraft",
        "/x/PaintCraft/Contents/MacOS/paintcraft",
    ] {
        let p = Platform::from_input(&input(Os::Macos, Arch::Arm64, exe));
        assert_eq!(
            p.install,
            InstallKind::Manual(ManualReason::Packaged),
            "{exe}"
        );
        assert!(p.asset_name(&version("0.1.1")).is_none());
    }
}

#[test]
fn windows_msi_is_the_per_machine_install_folder() {
    let mut i = input(
        Os::Windows,
        Arch::X64,
        "C:/Program Files/PaintCraft/paintcraft.exe",
    );
    i.program_files = vec!["C:/Program Files".into()];
    let p = Platform::from_input(&i);
    assert_eq!(
        p.install,
        InstallKind::WindowsMsi {
            dir: "C:/Program Files/PaintCraft".into()
        }
    );
    assert_eq!(
        p.asset_name(&version("0.1.1")).unwrap(),
        "paintcraft-0.1.1-windows-x64.msi"
    );
    // Case and separators do not matter on Windows.
    i.exe = Some("c:/program files/PAINTCRAFT/paintcraft.exe".into());
    assert!(matches!(
        Platform::from_input(&i).install,
        InstallKind::WindowsMsi { .. }
    ));
}

#[test]
fn windows_elsewhere_is_portable() {
    let mut i = input(
        Os::Windows,
        Arch::Arm64,
        "D:/tools/paintcraft-0.1.0-windows-arm64/bin/paintcraft.exe",
    );
    i.program_files = vec!["C:/Program Files".into()];
    let p = Platform::from_input(&i);
    assert!(matches!(p.install, InstallKind::WindowsPortable { .. }));
    assert_eq!(
        p.asset_name(&version("0.1.1")).unwrap(),
        "paintcraft-0.1.1-windows-arm64-portable.zip"
    );
    // A sibling folder that merely starts with the same text is not the MSI folder.
    i.exe = Some("C:/Program Files/PaintCraft2/paintcraft.exe".into());
    assert!(matches!(
        Platform::from_input(&i).install,
        InstallKind::WindowsPortable { .. }
    ));
}

#[test]
fn appimage_needs_an_existing_absolute_file() {
    let dir = support::tempdir();
    let mut header = [0u8; 64];
    header[..7].copy_from_slice(&[0x7f, b'E', b'L', b'F', 2, 1, 1]);
    header[16..18].copy_from_slice(&2u16.to_le_bytes());
    header[18..20].copy_from_slice(&62u16.to_le_bytes());
    let mount = dir.path().join("mounted");
    std::fs::create_dir_all(mount.join("usr/bin")).unwrap();
    let exe = support::write(mount.join("usr/bin/paintcraft"), &header);
    header[8..11].copy_from_slice(&[b'A', b'I', 2]);
    let image = support::write(dir.path().join("PaintCraft.AppImage"), &header);
    let mut i = input(Os::Linux, Arch::X64, exe.to_str().unwrap());
    i.appdir = Some(mount.into_os_string());
    i.appimage = Some(image.clone().into_os_string());
    let p = Platform::from_input(&i);
    assert_eq!(
        p.install,
        InstallKind::LinuxAppImage {
            path: image.canonicalize().unwrap()
        }
    );
    assert_eq!(
        p.asset_name(&version("0.1.1")).unwrap(),
        "paintcraft-0.1.1-linux-x64.AppImage"
    );
    for bad in [
        OsString::new(),
        "relative/PaintCraft.AppImage".into(),
        dir.path().join("missing").into_os_string(),
        dir.path().into(),
    ] {
        i.appimage = Some(bad.clone());
        assert!(
            matches!(Platform::from_input(&i).install, InstallKind::Manual(_)),
            "{bad:?}"
        );
    }
    i.appimage = None;
    assert_eq!(
        Platform::from_input(&i).install,
        InstallKind::Manual(ManualReason::Packaged)
    );
}

#[test]
fn packaged_and_dev_builds_never_self_replace() {
    // deb/rpm/tarball: /usr/bin without $APPIMAGE.
    let p = Platform::from_input(&input(Os::Linux, Arch::X64, "/usr/bin/paintcraft"));
    assert_eq!(p.install, InstallKind::Manual(ManualReason::Packaged));
    // A debug build, even inside a bundle.
    let mut i = input(
        Os::Macos,
        Arch::Arm64,
        "/Applications/PaintCraft.app/Contents/MacOS/paintcraft",
    );
    i.debug_build = true;
    assert_eq!(
        Platform::from_input(&i).install,
        InstallKind::Manual(ManualReason::DevBuild)
    );
}

#[test]
fn a_cargo_target_directory_is_a_dev_build() {
    let dir = support::tempdir();
    let release = dir.path().join("target/release");
    std::fs::create_dir_all(&release).unwrap();
    support::write(
        dir.path().join("target/CACHEDIR.TAG"),
        b"Signature: 8a477f597d28d172789f06886806bc55",
    );
    let exe = release.join("paintcraft");
    let mut i = input(Os::Linux, Arch::X64, exe.to_str().unwrap());
    i.appimage = Some(support::write(dir.path().join("a.AppImage"), b"x").into_os_string());
    assert_eq!(
        Platform::from_input(&i).install,
        InstallKind::Manual(ManualReason::DevBuild)
    );
}

#[test]
fn unsupported_architecture_or_os_is_manual() {
    let p = Platform::from_input(&input(Os::Linux, Arch::Other, "/usr/bin/paintcraft"));
    assert!(matches!(p.install, InstallKind::Manual(_)));
    assert!(p.id().is_none());
    let p = Platform::from_input(&input(Os::Other, Arch::X64, "/x/relay"));
    assert_eq!(
        p.install,
        InstallKind::Manual(ManualReason::UnsupportedPlatform)
    );
}

#[test]
fn every_platform_id_matches_the_packager() {
    let ids = [
        (Os::Windows, Arch::X64, "windows-x64"),
        (Os::Windows, Arch::Arm64, "windows-arm64"),
        (Os::Macos, Arch::Arm64, "macos-arm64"),
        (Os::Macos, Arch::X64, "macos-x64"),
        (Os::Linux, Arch::X64, "linux-x64"),
        (Os::Linux, Arch::Arm64, "linux-arm64"),
    ];
    for (os, arch, id) in ids {
        let p = Platform {
            os,
            arch,
            install: InstallKind::Manual(ManualReason::Packaged),
        };
        assert_eq!(p.id(), Some(id));
    }
}

#[test]
fn nested_editor_resolves_outer_installation_not_inner_editor_app() {
    let p = Platform::from_input(&input(
        Os::Macos,
        Arch::Arm64,
        "/Applications/PaintCraft.app/Contents/Helpers/PaintCraft Editor.app/Contents/MacOS/paintcraft",
    ));
    assert_eq!(
        p.install,
        InstallKind::MacosBundle {
            bundle: "/Applications/PaintCraft.app".into()
        }
    );
}
