//! Real temporary ELF headers and canonical identities, no AppImage execution.
use paintcraft_update::{Arch, DetectInput, InstallKind, ManualReason, Os, Platform};
use std::path::PathBuf;
fn header(machine: u16) -> [u8; 64] {
    let mut h = [0u8; 64];
    h[..7].copy_from_slice(&[0x7f, b'E', b'L', b'F', 2, 1, 1]);
    h[16..18].copy_from_slice(&2u16.to_le_bytes());
    h[18..20].copy_from_slice(&machine.to_le_bytes());
    h
}
fn fixture(arch: Arch) -> (tempfile::TempDir, DetectInput) {
    let temp = tempfile::tempdir().unwrap();
    let mount = temp.path().join("mounted");
    std::fs::create_dir_all(mount.join("usr/bin")).unwrap();
    let machine = match arch {
        Arch::X64 => 62,
        Arch::Arm64 => 183,
        Arch::Other => 0,
    };
    let mut bytes = header(machine);
    let exe = mount.join("usr/bin/paintcraft");
    std::fs::write(&exe, bytes).unwrap();
    bytes[8..11].copy_from_slice(&[b'A', b'I', 2]);
    let image = temp.path().join("PaintCraft.AppImage");
    std::fs::write(&image, bytes).unwrap();
    (
        temp,
        DetectInput {
            os: Os::Linux,
            arch,
            exe: Some(exe),
            appimage: Some(image.into_os_string()),
            appdir: Some(mount.into_os_string()),
            program_files: vec![],
            debug_build: false,
        },
    )
}
fn manual(input: &DetectInput) {
    assert_eq!(
        Platform::from_input(input).install,
        InstallKind::Manual(ManualReason::Packaged)
    );
}
#[test]
fn both_packaged_architectures_require_matching_runtime_and_program() {
    for arch in [Arch::X64, Arch::Arm64] {
        let (_temp, input) = fixture(arch);
        assert_eq!(
            Platform::from_input(&input).install,
            InstallKind::LinuxAppImage {
                path: std::fs::canonicalize(PathBuf::from(input.appimage.as_ref().unwrap()))
                    .unwrap()
            }
        );
        let mut foreign = input.clone();
        foreign.arch = match arch {
            Arch::X64 => Arch::Arm64,
            _ => Arch::X64,
        };
        manual(&foreign);
    }
}
#[test]
fn inherited_foreign_appimage_or_same_basename_cannot_select_a_system_install() {
    let (temp, mut input) = fixture(Arch::X64);
    let foreign = temp.path().join("foreign/usr/bin/paintcraft");
    std::fs::create_dir_all(foreign.parent().unwrap()).unwrap();
    std::fs::write(&foreign, header(62)).unwrap();
    input.exe = Some(foreign);
    manual(&input);
}
#[test]
fn appdir_must_exist_be_absolute_and_name_the_expected_program() {
    let (temp, input) = fixture(Arch::X64);
    for dir in [
        None,
        Some("".into()),
        Some("relative".into()),
        Some(temp.path().join("absent").into_os_string()),
        input.appimage.clone(),
    ] {
        let mut bad = input.clone();
        bad.appdir = dir;
        manual(&bad);
    }
}
#[test]
fn runtime_and_program_headers_are_checked_independently() {
    for runtime in [true, false] {
        for (index, value) in [(0, 0), (4, 1), (5, 2), (6, 0), (7, 255), (16, 1), (18, 0)] {
            let (_temp, input) = fixture(Arch::X64);
            let path = if runtime {
                PathBuf::from(input.appimage.as_ref().unwrap())
            } else {
                input.exe.clone().unwrap()
            };
            let mut bytes = std::fs::read(&path).unwrap();
            bytes[index] = value;
            std::fs::write(path, bytes).unwrap();
            manual(&input);
        }
    }
    for marker in [[b'A', b'I', 1], [0, 0, 0], [b'A', b'X', 2]] {
        let (_temp, input) = fixture(Arch::X64);
        let image = PathBuf::from(input.appimage.as_ref().unwrap());
        let mut bytes = std::fs::read(&image).unwrap();
        bytes[8..11].copy_from_slice(&marker);
        std::fs::write(image, bytes).unwrap();
        manual(&input);
    }
    let (_temp, input) = fixture(Arch::X64);
    std::fs::write(PathBuf::from(input.appimage.as_ref().unwrap()), b"\x7fELF").unwrap();
    manual(&input);
}
#[test]
#[cfg(unix)]
fn canonical_aliases_are_safe_but_runtime_links_and_program_escape_are_not() {
    use std::os::unix::fs::symlink;
    let (temp, input) = fixture(Arch::X64);
    let mount = PathBuf::from(input.appdir.as_ref().unwrap());
    let alias = temp.path().join("mount-alias");
    symlink(&mount, &alias).unwrap();
    let exe_alias = temp.path().join("exe-alias");
    symlink(input.exe.as_ref().unwrap(), &exe_alias).unwrap();
    let mut canonical = input.clone();
    canonical.appdir = Some(alias.into_os_string());
    canonical.exe = Some(exe_alias);
    assert!(matches!(
        Platform::from_input(&canonical).install,
        InstallKind::LinuxAppImage { .. }
    ));
    let image_alias = temp.path().join("image-alias");
    symlink(
        PathBuf::from(input.appimage.as_ref().unwrap()),
        &image_alias,
    )
    .unwrap();
    let mut bad = input.clone();
    bad.appimage = Some(image_alias.into_os_string());
    manual(&bad);
    let outside = temp.path().join("foreign-engine");
    std::fs::write(&outside, header(62)).unwrap();
    std::fs::remove_file(input.exe.as_ref().unwrap()).unwrap();
    symlink(&outside, input.exe.as_ref().unwrap()).unwrap();
    let mut escape = input;
    escape.exe = Some(outside);
    manual(&escape);
}
