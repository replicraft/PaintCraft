//! Whole AppImage replacement with an on-disk original retained for recovery.
use super::{Relaunch, random_suffix};
use crate::error::{Error, Result};
use semver::Version;
use std::{
    fs,
    path::{Path, PathBuf},
};

/// Gives the download safe executable permissions and replaces the image, preserving a backup.
pub fn swap(payload: &Path, target: &Path) -> Result<PathBuf> {
    use std::os::unix::fs::PermissionsExt;
    let old = fs::symlink_metadata(target).map_err(Error::io("read current AppImage"))?;
    if !old.is_file() {
        return Err(Error::BundleInvalid(
            "AppImage is not a regular file".into(),
        ));
    }
    let mode = (old.permissions().mode() & 0o755) | 0o100;
    fs::set_permissions(payload, fs::Permissions::from_mode(mode))
        .map_err(Error::io("set AppImage permissions"))?;
    let name = target
        .file_name()
        .ok_or_else(|| Error::BundleInvalid("AppImage has no filename".into()))?;
    let mut backup_name = name.to_os_string();
    backup_name.push(format!(".old-{}", random_suffix()));
    let backup = target.with_file_name(backup_name);
    fs::rename(target, &backup).map_err(Error::io("retain original AppImage"))?;
    if let Err(error) = fs::rename(payload, target) {
        fs::rename(&backup, target).map_err(Error::io(
            "restore original AppImage after failed replacement",
        ))?;
        return Err(Error::Io {
            context: "install new AppImage",
            source: error,
        });
    }
    Ok(backup)
}

/// Installs the verified image and schedules restart after the caller exits.
pub fn apply(target: &Path, payload: &Path, version: &Version) -> Result<Relaunch> {
    let backup = swap(payload, target)?;
    if let Err(error) = crate::helper::relaunch_exec(std::process::id(), target) {
        fs::rename(target, payload).map_err(Error::io("set aside failed AppImage update"))?;
        fs::rename(&backup, target)
            .map_err(Error::io("restore original AppImage after helper failure"))?;
        return Err(error);
    }
    Ok(Relaunch {
        version: version.clone(),
        launch: target.to_path_buf(),
    })
}
