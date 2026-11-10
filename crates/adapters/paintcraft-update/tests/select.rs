//! Which release and which asset are accepted, from the JSON alone.

use paintcraft_update::{Arch, InstallKind, ManualReason, Os, Plan, Platform, ReleaseDoc, Updater};

mod support;
use support::{REPO, release_json};

const BASE: &str = "https://github.com";
const MAC_ASSET: &str = "paintcraft-0.1.1-macos-arm64-update.zip";

fn updater(current: &str, os: Os, arch: Arch, install: InstallKind) -> Updater {
    Updater::new(current, Platform { os, arch, install }).unwrap()
}

fn doc(tag: &str, prerelease: bool, assets: &[(&str, usize)]) -> ReleaseDoc {
    paintcraft_update::release::parse(release_json(BASE, tag, prerelease, assets).as_bytes())
        .unwrap()
}

fn mac() -> Updater {
    updater(
        "0.1.0",
        Os::Macos,
        Arch::Arm64,
        InstallKind::MacosBundle {
            bundle: "/Applications/PaintCraft.app".into(),
        },
    )
}

fn mac_assets() -> Vec<(&'static str, usize)> {
    vec![
        (MAC_ASSET, 1000),
        ("paintcraft-0.1.1-macos-arm64-update.zip.minisig", 300),
        ("paintcraft-0.1.1-linux-x64.AppImage", 5),
    ]
}

#[test]
fn the_asset_for_this_platform_is_chosen_with_its_signature() {
    let available = mac()
        .available_from(&doc("v0.1.1", false, &mac_assets()))
        .unwrap()
        .unwrap();
    assert_eq!(available.version.to_string(), "0.1.1");
    assert_eq!(available.asset_name.as_deref(), Some(MAC_ASSET));
    assert_eq!(available.asset_size, Some(1000));
    assert_eq!(
        available.release_url,
        format!("{BASE}/{REPO}/releases/tag/v0.1.1")
    );
    assert!(matches!(
        available.plan,
        Plan::InPlace(InstallKind::MacosBundle { .. })
    ));
}

#[test]
fn each_platform_picks_its_own_file() {
    let all: Vec<(String, usize)> = [
        "windows-x64.msi",
        "windows-x64-portable.zip",
        "windows-arm64.msi",
        "windows-arm64-portable.zip",
        "macos-arm64-update.zip",
        "macos-x64-update.zip",
        "linux-x64.AppImage",
        "linux-arm64.AppImage",
        "linux-x64.deb",
        "linux-x64.tar.gz",
    ]
    .iter()
    .flat_map(|suffix| {
        let name = format!("paintcraft-0.1.1-{suffix}");
        [(format!("{name}.minisig"), 300), (name, 1000)]
    })
    .collect();
    let refs: Vec<(&str, usize)> = all.iter().map(|(n, s)| (n.as_str(), *s)).collect();
    let release = doc("v0.1.1", false, &refs);
    let cases = [
        (
            Os::Windows,
            Arch::X64,
            InstallKind::WindowsMsi {
                dir: "C:/Program Files/PaintCraft".into(),
            },
            "paintcraft-0.1.1-windows-x64.msi",
        ),
        (
            Os::Windows,
            Arch::Arm64,
            InstallKind::WindowsPortable {
                dir: "D:/r/bin".into(),
            },
            "paintcraft-0.1.1-windows-arm64-portable.zip",
        ),
        (
            Os::Macos,
            Arch::X64,
            InstallKind::MacosBundle {
                bundle: "/Applications/PaintCraft.app".into(),
            },
            "paintcraft-0.1.1-macos-x64-update.zip",
        ),
        (
            Os::Linux,
            Arch::Arm64,
            InstallKind::LinuxAppImage {
                path: "/home/u/PaintCraft.AppImage".into(),
            },
            "paintcraft-0.1.1-linux-arm64.AppImage",
        ),
    ];
    for (os, arch, install, expected) in cases {
        let available = updater("0.1.0", os, arch, install)
            .available_from(&release)
            .unwrap()
            .unwrap();
        assert_eq!(available.asset_name.as_deref(), Some(expected));
    }
}

#[test]
fn a_manual_install_gets_the_link_only() {
    let u = updater(
        "0.1.0",
        Os::Linux,
        Arch::X64,
        InstallKind::Manual(ManualReason::Packaged),
    );
    let available = u
        .available_from(&doc("v0.1.1", false, &[]))
        .unwrap()
        .unwrap();
    assert!(available.asset_name.is_none());
    assert_eq!(
        available.plan,
        Plan::Manual {
            release_url: format!("{BASE}/{REPO}/releases/tag/v0.1.1")
        }
    );
    let result = u.download(&available, &mut |_| std::ops::ControlFlow::Continue(()));
    assert!(matches!(result, Err(paintcraft_update::Error::ManualOnly)));
}

#[test]
fn equal_older_draft_and_prerelease_offer_nothing() {
    let u = mac();
    assert!(
        u.available_from(&doc("v0.1.0", false, &mac_assets()))
            .unwrap()
            .is_none()
    );
    assert!(
        u.available_from(&doc("v0.0.9", false, &mac_assets()))
            .unwrap()
            .is_none()
    );
    assert!(
        u.available_from(&doc("v0.2.0-rc.1", true, &[]))
            .unwrap()
            .is_none()
    );
    let mut draft = doc("v0.1.1", false, &mac_assets());
    draft.draft = true;
    assert!(u.available_from(&draft).unwrap().is_none());
}

#[test]
fn a_prerelease_build_may_be_offered_a_prerelease() {
    let u = updater(
        "0.2.0-rc.1",
        Os::Macos,
        Arch::Arm64,
        InstallKind::MacosBundle {
            bundle: "/Applications/PaintCraft.app".into(),
        },
    );
    let assets = [
        ("paintcraft-0.2.0-rc.2-macos-arm64-update.zip", 10),
        ("paintcraft-0.2.0-rc.2-macos-arm64-update.zip.minisig", 300),
    ];
    let available = u
        .available_from(&doc("v0.2.0-rc.2", true, &assets))
        .unwrap()
        .unwrap();
    assert_eq!(available.version.to_string(), "0.2.0-rc.2");
}

#[test]
fn the_tag_must_agree_with_the_asset_names() {
    // Tag says 0.1.1 but the only assets are named for another version: no such asset, refused.
    for version in ["0.1.2", "0.0.9"] {
        let name = format!("paintcraft-{version}-macos-arm64-update.zip");
        let sig = format!("{name}.minisig");
        let release = doc("v0.1.1", false, &[(&name, 10), (&sig, 300)]);
        let result = mac().available_from(&release);
        assert!(
            matches!(result, Err(paintcraft_update::Error::AssetMissing(_))),
            "{result:?}"
        );
    }
    assert!(
        mac()
            .available_from(&doc("0.1.1", false, &mac_assets()))
            .is_err()
    );
}

#[test]
fn a_missing_signature_is_an_error_not_an_unsigned_install() {
    let release = doc("v0.1.1", false, &[(MAC_ASSET, 1000)]);
    assert!(matches!(
        mac().available_from(&release),
        Err(paintcraft_update::Error::SignatureMissing(_))
    ));
}

#[test]
fn urls_outside_the_release_are_refused() {
    let mut release = doc("v0.1.1", false, &mac_assets());
    release.assets[0].browser_download_url = "https://evil.example/relay.zip".into();
    assert!(mac().available_from(&release).is_err());
    let mut release = doc("v0.1.1", false, &mac_assets());
    release.assets[0].browser_download_url =
        format!("{BASE}/Someone/Else/releases/download/v0.1.1/{MAC_ASSET}");
    assert!(mac().available_from(&release).is_err());
}

#[test]
fn implausible_documents_are_refused() {
    let huge = paintcraft_update::release::MAX_ASSET_BYTES as usize + 1;
    for (size, sig) in [(0, 300), (huge, 300), (1000, 0), (1000, 100_000)] {
        let release = doc(
            "v0.1.1",
            false,
            &[
                (MAC_ASSET, size),
                ("paintcraft-0.1.1-macos-arm64-update.zip.minisig", sig),
            ],
        );
        assert!(mac().available_from(&release).is_err(), "{size} {sig}");
    }
    // The same asset listed twice is ambiguous.
    let release = doc(
        "v0.1.1",
        false,
        &[
            (MAC_ASSET, 5),
            (MAC_ASSET, 5),
            ("paintcraft-0.1.1-macos-arm64-update.zip.minisig", 300),
        ],
    );
    assert!(mac().available_from(&release).is_err());
    // The prerelease flag must agree with the tag.
    assert!(
        mac()
            .available_from(&doc("v0.1.1", true, &mac_assets()))
            .is_err()
    );
    assert!(paintcraft_update::release::parse(b"not json").is_err());
}

#[test]
fn a_foreign_release_page_is_replaced_by_the_tag_page() {
    let mut release = doc("v0.1.1", false, &mac_assets());
    release.html_url = "https://evil.example/phish".into();
    let available = mac().available_from(&release).unwrap().unwrap();
    assert_eq!(
        available.release_url,
        format!("{BASE}/{REPO}/releases/tag/v0.1.1")
    );
}
