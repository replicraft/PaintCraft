//! Replacing the running installation. Each platform module has pure, separately tested steps
//! (prepare, swap, roll back) and one thin function that starts the relaunch helper.

use std::path::PathBuf;

use semver::Version;

#[cfg(unix)]
pub mod linux;
#[cfg(unix)]
pub mod macos;
pub mod windows;

/// What the caller must do after a successful [`crate::Updater::apply`]: quit this process now.
/// A detached helper has already been started; it waits for this process to exit (up to five
/// minutes) and then starts the new version, so the app must not start another copy itself.
/// All application codec/background work must already have been joined by the caller.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Relaunch {
    pub version: Version,
    /// What the helper will start.
    pub launch: PathBuf,
}

/// A fresh unpredictable suffix for names that sit beside the installation.
pub(crate) fn random_suffix() -> String {
    uuid::Uuid::new_v4().simple().to_string()
}
