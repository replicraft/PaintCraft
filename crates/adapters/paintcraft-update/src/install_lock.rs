//! Cross-process installation locks live outside the installation that gets replaced.
use crate::{Error, InstallKind, Platform, Result};
use sha2::{Digest, Sha256};
use std::{
    fs::{self, File, OpenOptions, TryLockError},
    path::{Path, PathBuf},
};

/// A non-clonable shared lease retained for the entire desktop process lifetime.
/// After an install attempt becomes exclusive, the process must quit even on error.
#[derive(Debug)]
pub struct InstallLock {
    file: File,
    identity: PathBuf,
    exclusive: bool,
    lost: bool,
}
impl InstallLock {
    /// Takes a shared lease for a recognized installation in the user's shared-support directory.
    /// Every editor process must call this before loading documents or starting background work.
    pub fn acquire(platform: &Platform) -> Result<Option<Self>> {
        let Some(identity) = identity(platform)? else {
            return Ok(None);
        };
        Self::at(&support_directory()?, identity).map(Some)
    }
    fn at(directory: &Path, identity: PathBuf) -> Result<Self> {
        fs::create_dir_all(directory).map_err(Error::io("create installation lock directory"))?;
        let hash = Sha256::digest(identity.to_string_lossy().as_bytes());
        let name: String = hash.iter().map(|byte| format!("{byte:02x}")).collect();
        let file = OpenOptions::new()
            .read(true)
            .write(true)
            .create(true)
            .truncate(false)
            .open(directory.join(format!("{name}.lock")))
            .map_err(Error::io("open installation lock"))?;
        file.try_lock_shared().map_err(lock_error)?;
        Ok(Self {
            file,
            identity,
            exclusive: false,
            lost: false,
        })
    }
    /// True once this process can no longer safely resume editing after an apply attempt.
    pub fn requires_exit(&self) -> bool {
        self.exclusive || self.lost
    }
    pub(crate) fn begin_install(&mut self, platform: &Platform) -> Result<()> {
        if self.requires_exit() || identity(platform)?.as_ref() != Some(&self.identity) {
            return Err(Error::InstallationBusy);
        }
        self.file
            .unlock()
            .map_err(Error::io("release own editor lease"))?;
        match self.file.try_lock() {
            Ok(()) => {
                self.exclusive = true;
                Ok(())
            }
            Err(error) => {
                if self.file.try_lock_shared().is_err() {
                    self.lost = true;
                    return Err(Error::InstallationLockLost);
                }
                Err(lock_error(error))
            }
        }
    }
    /// Isolated lock root for tests only; no production caller enables this feature.
    #[cfg(any(test, feature = "test-util"))]
    pub fn for_test(platform: &Platform, directory: &Path) -> Result<Self> {
        let identity = identity(platform)?.ok_or(Error::ManualOnly)?;
        Self::at(directory, identity)
    }
}
fn lock_error(error: TryLockError) -> Error {
    match error {
        TryLockError::WouldBlock => Error::InstallationBusy,
        TryLockError::Error(source) => Error::Io {
            context: "lock installation",
            source,
        },
    }
}
fn identity(platform: &Platform) -> Result<Option<PathBuf>> {
    let path = match &platform.install {
        InstallKind::MacosBundle { bundle } => bundle,
        InstallKind::LinuxAppImage { path } => path,
        InstallKind::WindowsPortable { dir } | InstallKind::WindowsMsi { dir } => dir,
        InstallKind::Manual(_) => return Ok(None),
    };
    fs::canonicalize(path)
        .map(Some)
        .map_err(Error::io("identify installed application"))
}
fn support_directory() -> Result<PathBuf> {
    let base = if cfg!(windows) {
        std::env::var_os("LOCALAPPDATA").map(PathBuf::from)
    } else if cfg!(target_os = "macos") {
        std::env::var_os("HOME").map(|home| PathBuf::from(home).join("Library/Application Support"))
    } else {
        std::env::var_os("HOME").map(|home| PathBuf::from(home).join(".local/state"))
    }
    .ok_or_else(|| Error::Helper("user shared-support directory is unavailable".into()))?;
    Ok(base.join("PaintCraft/update-locks"))
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn another_editor_prevents_exclusive_and_restores_shared() {
        let temp = tempfile::tempdir().unwrap();
        let install = temp.path().join("PaintCraft.AppImage");
        fs::write(&install, b"old").unwrap();
        let p = Platform {
            os: crate::Os::Linux,
            arch: crate::Arch::X64,
            install: InstallKind::LinuxAppImage { path: install },
        };
        let locks = temp.path().join("support");
        let mut first = InstallLock::for_test(&p, &locks).unwrap();
        let second = InstallLock::for_test(&p, &locks).unwrap();
        assert!(matches!(
            first.begin_install(&p),
            Err(Error::InstallationBusy)
        ));
        assert!(!first.requires_exit());
        drop(second);
        first.begin_install(&p).unwrap();
        assert!(first.requires_exit());
        assert!(matches!(
            InstallLock::for_test(&p, &locks),
            Err(Error::InstallationBusy)
        ));
        drop(first);
        assert!(InstallLock::for_test(&p, &locks).is_ok());
    }
    #[test]
    fn lock_inode_survives_installation_replacement() {
        let temp = tempfile::tempdir().unwrap();
        let install = temp.path().join("PaintCraft.app");
        fs::create_dir(&install).unwrap();
        let p = Platform {
            os: crate::Os::Macos,
            arch: crate::Arch::X64,
            install: InstallKind::MacosBundle {
                bundle: install.clone(),
            },
        };
        let locks = temp.path().join("support");
        let mut lock = InstallLock::for_test(&p, &locks).unwrap();
        lock.begin_install(&p).unwrap();
        fs::rename(&install, temp.path().join("old.app")).unwrap();
        fs::create_dir(&install).unwrap();
        assert!(matches!(
            InstallLock::for_test(&p, &locks),
            Err(Error::InstallationBusy)
        ));
        drop(lock);
        assert!(InstallLock::for_test(&p, &locks).is_ok());
        assert_eq!(fs::read_dir(locks).unwrap().count(), 1);
    }
}
