//! Windows. UNVERIFIED on a real Windows machine: the path, archive and rename logic is tested on
//! the build host; the PowerShell relaunch, `msiexec` and the elevation prompt are not.
//!
//! * MSI (`WindowsMsi`, per-machine): after this process exits a hidden PowerShell runs
//!   `msiexec /i <msi> /passive /norestart` elevated (one UAC prompt) and starts PaintCraft again.
//!   The package is `MajorUpgrade` with the fixed UpgradeCode of `packaging/windows/windows.wxs`.
//! * Portable (`WindowsPortable`): the executables are extracted next to the running ones; each
//!   running `.exe` is renamed aside (Windows allows renaming a running image, not overwriting
//!   it) and the new file takes its name.

use std::fs;
use std::path::{Path, PathBuf};

use super::random_suffix;
use crate::error::{Error, Result};
use crate::zipx::{self, Limits};

/// Complete matched executable/manifest group in the flat PaintCraft portable ZIP.
pub const PROGRAMS: [&str; 3] = [
    "paintcraft.exe",
    "paintcraft-codec-worker.exe",
    "paintcraft-codec-worker.json",
];
const NOTICES: [&str; 6] = [
    "LICENSE",
    "ThirdPartyLicenses.txt",
    "FONT_LICENSES.txt",
    "JPEGXR_LICENSE.txt",
    "DAV1D_LICENSE.txt",
    "HDR_EXR_LICENSES_V1.txt",
];

/// Returns the version/platform stem of a portable asset name.
pub fn portable_top(asset_name: &str) -> Option<&str> {
    asset_name
        .strip_suffix("-portable.zip")
        .filter(|name| !name.is_empty())
}

/// Extracts the signed flat archive and requires the entire matched codec group.
pub fn prepare_portable(zip: &Path, staging: &Path, asset_name: &str) -> Result<PathBuf> {
    portable_top(asset_name).ok_or_else(|| Error::BundleInvalid("unexpected asset name".into()))?;
    let target = staging.join("extracted");
    fs::create_dir(&target).map_err(Error::io("create the extraction directory"))?;
    zipx::extract(zip, &target, "", &Limits::APP)?;
    let mut found = Vec::new();
    for entry in fs::read_dir(&target).map_err(Error::io("read portable payload"))? {
        let entry = entry.map_err(Error::io("read portable entry"))?;
        let name = entry.file_name().to_string_lossy().into_owned();
        if !entry
            .file_type()
            .map_err(Error::io("inspect portable entry"))?
            .is_file()
            || !(PROGRAMS.contains(&name.as_str()) || NOTICES.contains(&name.as_str()))
        {
            return Err(Error::BundleInvalid(format!(
                "unexpected portable entry {name:?}"
            )));
        }
        found.push(name);
    }
    if PROGRAMS
        .iter()
        .any(|name| !found.iter().any(|got| got == name))
    {
        return Err(Error::BundleInvalid(
            "incomplete executable/helper/manifest group".into(),
        ));
    }
    Ok(target)
}

/// One completed multi-file replacement, retaining every new and old file for rollback.
#[derive(Debug)]
pub struct PortableSwap {
    placed: Vec<PathBuf>,
    backups: Vec<(PathBuf, PathBuf)>,
}
impl PortableSwap {
    /// Retained originals, for recovery; no backup is automatically deleted.
    pub fn backups(&self) -> impl Iterator<Item = &PathBuf> {
        self.backups.iter().map(|(_, old)| old)
    }
    /// Removes every installed file, including additions, before restoring all originals.
    pub fn rollback(self) -> Result<()> {
        for path in self.placed.iter().rev() {
            fs::remove_file(path).map_err(Error::io("remove new portable file during rollback"))?;
        }
        for (target, old) in self.backups.iter().rev() {
            fs::rename(old, target).map_err(Error::io("restore original portable file"))?;
        }
        Ok(())
    }
}

/// Replaces the complete matched group and notices. On failure restores every old file.
/// The caller must first stop and join all application background/codec work.
pub fn swap_portable(install_dir: &Path, new_bin: &Path) -> Result<PortableSwap> {
    for name in PROGRAMS {
        if !fs::symlink_metadata(new_bin.join(name)).is_ok_and(|m| m.is_file()) {
            return Err(Error::BundleInvalid(
                "incomplete portable group at install".into(),
            ));
        }
    }
    let mut asides: Vec<(PathBuf, PathBuf)> = Vec::new();
    let mut placed: Vec<PathBuf> = Vec::new();
    let result = (|| {
        for name in PROGRAMS.into_iter().chain(NOTICES) {
            let source = new_bin.join(name);
            if !source.exists() {
                continue;
            }
            let target = install_dir.join(name);
            if target.exists() {
                if !fs::symlink_metadata(&target)
                    .map_err(Error::io("inspect installed entry"))?
                    .is_file()
                {
                    return Err(Error::BundleInvalid(
                        "installed entry is not a plain file".into(),
                    ));
                }
                let old = install_dir.join(format!("{name}.old-{}", random_suffix()));
                fs::rename(&target, &old).map_err(Error::io("move old program aside"))?;
                asides.push((target.clone(), old));
            }
            fs::rename(&source, &target).map_err(Error::io("install new portable entry"))?;
            placed.push(target);
        }
        Ok(())
    })();
    if let Err(error) = result {
        for target in placed.iter().rev() {
            fs::remove_file(target).map_err(Error::io("remove failed update during rollback"))?;
        }
        for (target, old) in asides.iter().rev() {
            fs::rename(old, target).map_err(Error::io("restore original portable installation"))?;
        }
        return Err(error);
    }
    Ok(PortableSwap {
        placed,
        backups: asides,
    })
}

/// Removes `<name>.old-<32 hex>` files of the programs in `install_dir`.
pub fn cleanup_stale(install_dir: &Path) {
    let Ok(entries) = fs::read_dir(install_dir) else {
        return;
    };
    for entry in entries.flatten() {
        let file = entry.file_name().to_string_lossy().into_owned();
        let ours = PROGRAMS.iter().any(|program| {
            file.strip_prefix(&format!("{program}.old-"))
                .is_some_and(|s| s.len() == 32 && s.bytes().all(|b| b.is_ascii_hexdigit()))
        });
        if ours && entry.file_type().is_ok_and(|t| t.is_file()) {
            let _ = fs::remove_file(entry.path());
        }
    }
}

#[cfg(windows)]
use semver::Version;

#[cfg(windows)]
pub fn apply_msi(
    dir: &Path,
    msi: &Path,
    sha256_hex: &str,
    staging: PathBuf,
    version: &Version,
) -> Result<super::Relaunch> {
    let launch = dir.join("paintcraft.exe");
    // The helper needs the MSI after this process is gone, so the directory is kept; it deletes it.
    crate::helper::powershell_relaunch(
        std::process::id(),
        &launch,
        Some((msi, sha256_hex)),
        &[],
        Some(&staging),
    )?;
    Ok(super::Relaunch {
        version: version.clone(),
        launch,
    })
}

#[cfg(windows)]
pub fn apply_portable(
    dir: &Path,
    zip: &Path,
    staging: PathBuf,
    asset_name: &str,
    version: &Version,
) -> Result<super::Relaunch> {
    let new_bin = prepare_portable(zip, &staging, asset_name)?;
    let target = if asset_name.ends_with("-windows-arm64-portable.zip") {
        "aarch64-pc-windows-msvc"
    } else {
        "x86_64-pc-windows-msvc"
    };
    crate::payload::codec_group(&new_bin, &new_bin, version, target)?;
    let old = swap_portable(dir, &new_bin)?;
    let launch = dir.join("paintcraft.exe");
    let olds: Vec<&Path> = old.backups().map(PathBuf::as_path).collect();
    if let Err(error) =
        crate::helper::powershell_relaunch(std::process::id(), &launch, None, &olds, Some(&staging))
    {
        old.rollback()?;
        return Err(error);
    }
    Ok(super::Relaunch {
        version: version.clone(),
        launch,
    })
}
