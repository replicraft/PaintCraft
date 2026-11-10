//! Signed self-update for the PaintCraft desktop app. See the crate README for installation guarantees.
//!
//! # Examples
//!
//! ```no_run
//! use paintcraft_update::Updater;
//! use std::ops::ControlFlow;
//! # fn main() -> paintcraft_update::Result<()> {
//! let platform = paintcraft_update::Platform::detect();
//! let mut installation = paintcraft_update::InstallLock::acquire(&platform)?;
//! // Retain installation for this entire editor process lifetime.
//! let updater = Updater::new("0.1.0",platform)?;
//! if let Some(available) = updater.check()? {                  // one GET: releases/latest
//!     let verified = updater.download(&available, &mut |p| ControlFlow::Continue(()))?;
//!     let install = installation.as_mut().ok_or(paintcraft_update::Error::ManualOnly)?;
//!     let relaunch = updater.apply(verified, install)?;                  // swap on disk, start helper
//!     // quit now: the helper starts the new version once this process has exited
//! }
//! # Ok(())
//! # }
//! ```
//!
//! The API is blocking: call it from a worker thread.

pub mod apply;
mod error;
mod install_lock;
mod payload;
pub use install_lock::InstallLock;
pub mod helper;
pub mod http;
mod platform;
#[cfg(unix)]
mod plist;
pub mod release;
pub mod verify;
pub mod version;
pub mod zipx;

use std::fs::{self, OpenOptions};
use std::io::{Read, Write};
use std::ops::ControlFlow;
use std::path::{Path, PathBuf};

use semver::Version;
use sha2::{Digest, Sha256};

pub use apply::Relaunch;
pub use error::{Error, Result};
pub use http::Endpoint;
pub use platform::{Arch, DetectInput, InstallKind, ManualReason, Os, Platform};
pub use release::ReleaseDoc;

/// The repository releases are read from. Compiled in; no setting, file or environment variable
/// can change it.
pub const REPOSITORY: &str = "replicraft/PaintCraft";

/// Prefix of the directories downloads are staged in.
const STAGING_PREFIX: &str = ".paintcraft-update-";

/// What to do with an available release.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Plan {
    /// Download, verify and replace this installation.
    InPlace(InstallKind),
    /// Show the link; this installation is never replaced in place.
    Manual { release_url: String },
}

/// A newer release this build may install or show.
#[derive(Debug, Clone)]
#[non_exhaustive]
pub struct Available {
    pub version: Version,
    /// File name of the installer, `None` for [`Plan::Manual`].
    pub asset_name: Option<String>,
    /// Bytes of the installer as listed by the release.
    pub asset_size: Option<u64>,
    /// The release page (`html_url`), for the release notes.
    pub release_url: String,
    pub plan: Plan,
    package: Option<release::Package>,
}

/// Download progress; the callback may return `Break` to cancel.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Progress {
    pub received: u64,
    pub total: u64,
}

/// A downloaded installer whose signature and trusted comment verified. Owns a private staging
/// directory that is deleted on drop.
#[derive(Debug)]
pub struct Verified {
    dir: tempfile::TempDir,
    payload: PathBuf,
    version: Version,
    asset_name: String,
    sha256: [u8; 32],
    kind: InstallKind,
}

impl Verified {
    pub fn version(&self) -> &Version {
        &self.version
    }

    pub fn asset_name(&self) -> &str {
        &self.asset_name
    }

    pub fn payload(&self) -> &Path {
        &self.payload
    }

    pub fn staging_dir(&self) -> &Path {
        self.dir.path()
    }

    pub fn sha256_hex(&self) -> String {
        self.sha256.iter().map(|b| format!("{b:02x}")).collect()
    }

    /// The payload must still be the verified bytes: it sat in a private directory, but it is
    /// hashed again right before use.
    fn recheck(&self) -> Result<()> {
        let mut hasher = Sha256::new();
        let mut file = fs::File::open(&self.payload).map_err(Error::io("reopen the download"))?;
        let mut buffer = [0_u8; 32 * 1024];
        loop {
            let count = file
                .read(&mut buffer)
                .map_err(Error::io("reread the download"))?;
            if count == 0 {
                break;
            }
            hasher.update(&buffer[..count]);
        }
        if hasher.finalize().as_slice() == self.sha256 {
            Ok(())
        } else {
            Err(Error::PayloadChanged)
        }
    }
}

/// Blocking updater configured for one installation and its trusted signing key.
pub struct Updater {
    current: Version,
    platform: Platform,
    client: http::Client,
    /// The key every download must verify against: the compiled-in release key, except in tests.
    key: minisign_verify::PublicKey,
}

impl std::fmt::Debug for Updater {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Updater")
            .field("current", &self.current)
            .field("platform", &self.platform)
            .finish_non_exhaustive()
    }
}

impl Updater {
    /// The production updater: GitHub over HTTPS, the compiled-in key, `PaintCraft/<version>` as the
    /// User-Agent. Nothing here reads the environment except platform detection.
    ///
    /// # Errors
    /// Returns an error if the current version or compiled signing key is malformed.
    pub fn new(current_version: &str, platform: Platform) -> Result<Self> {
        Self::build(
            current_version,
            platform,
            Endpoint::github(),
            verify::public_key()?,
        )
    }

    /// [`Updater::new`] with [`Platform::detect`].
    ///
    /// # Errors
    /// Returns an error if the version or signing key is malformed.
    pub fn detect(current_version: &str) -> Result<Self> {
        Self::new(current_version, Platform::detect())
    }

    /// Tests only: a local endpoint. Not compiled without the `test-util` feature, which no
    /// dependent enables.
    #[cfg(any(test, feature = "test-util"))]
    pub fn with_endpoint(
        current_version: &str,
        platform: Platform,
        endpoint: Endpoint,
    ) -> Result<Self> {
        let key = verify::decode_key(verify::TEST_ONLY_PUBLIC_KEY)?;
        Self::build(current_version, platform, endpoint, key)
    }

    /// Tests only: a local endpoint but the real compiled-in key (to prove test signatures fail).
    #[cfg(any(test, feature = "test-util"))]
    pub fn with_endpoint_release_key(
        current_version: &str,
        platform: Platform,
        endpoint: Endpoint,
    ) -> Result<Self> {
        Self::build(current_version, platform, endpoint, verify::public_key()?)
    }

    /// Untrusted comment of the key downloads are verified against.
    pub fn key_comment(&self) -> Option<&str> {
        self.key.untrusted_comment()
    }

    fn build(
        current_version: &str,
        platform: Platform,
        endpoint: Endpoint,
        key: minisign_verify::PublicKey,
    ) -> Result<Self> {
        let current = version::parse(current_version)
            .ok_or_else(|| Error::BadCurrentVersion(current_version.to_owned()))?;
        let client = http::Client::new(endpoint, &format!("PaintCraft/{current}"));
        Ok(Self {
            current,
            platform,
            client,
            key,
        })
    }

    pub fn platform(&self) -> &Platform {
        &self.platform
    }

    pub fn endpoint(&self) -> &Endpoint {
        self.client.endpoint()
    }

    /// Asks GitHub for the latest release (one request). `None`: nothing newer to offer.
    ///
    /// # Errors
    /// Returns an error if the request fails, the document is malformed, or a newer in-place
    /// release lacks the required installer or signature.
    #[tracing::instrument(name = "update_check", skip_all)]
    pub fn check(&self) -> Result<Option<Available>> {
        let url = format!(
            "{}/repos/{REPOSITORY}/releases/latest",
            self.client.endpoint().api_base
        );
        let body =
            self.client
                .get_bytes(&url, "application/vnd.github+json", release::MAX_API_BYTES)?;
        let doc = release::parse(&body)?;
        self.available_from(&doc)
    }

    /// The decision part of [`Updater::check`] on an already fetched document.
    ///
    /// # Errors
    /// Returns an error if release metadata, URLs, version tags, or required assets are invalid.
    pub fn available_from(&self, doc: &ReleaseDoc) -> Result<Option<Available>> {
        let endpoint = self.client.endpoint();
        let Some(selection) =
            release::select(doc, &self.current, &self.platform, endpoint, REPOSITORY)?
        else {
            return Ok(None);
        };
        let plan = match &self.platform.install {
            InstallKind::Manual(_) => Plan::Manual {
                release_url: selection.release_url.clone(),
            },
            kind => Plan::InPlace(kind.clone()),
        };
        Ok(Some(Available {
            version: selection.version,
            asset_name: selection.package.as_ref().map(|p| p.asset.name.clone()),
            asset_size: selection.package.as_ref().map(|p| p.asset.size),
            release_url: selection.release_url,
            plan,
            package: selection.package,
        }))
    }

    /// Downloads the installer and its signature into a private directory beside the install
    /// (same filesystem for the final rename) and verifies the signature while streaming.
    ///
    /// # Errors
    /// Returns an error for an incompatible plan, cancellation, network/I/O failure, a size
    /// mismatch, or an invalid signature or trusted comment. Partial staging data is removed.
    pub fn download(
        &self,
        available: &Available,
        progress: &mut dyn FnMut(Progress) -> ControlFlow<()>,
    ) -> Result<Verified> {
        let (Plan::InPlace(kind), Some(package)) = (&available.plan, &available.package) else {
            return Err(Error::ManualOnly);
        };
        self.ensure_newer(&available.version)?;
        if *kind != self.platform.install {
            return Err(Error::ManualOnly);
        }
        #[cfg(unix)]
        if let InstallKind::MacosBundle { bundle } = kind {
            apply::macos::preflight(bundle)?;
        }
        let dir = staging_dir(kind)?;
        let signature_text = self.client.get_bytes(
            &package.signature.url,
            "application/octet-stream",
            package.signature.size,
        )?;
        if signature_text.len() as u64 != package.signature.size {
            return Err(Error::SizeMismatch {
                expected: package.signature.size,
                received: signature_text.len() as u64,
            });
        }
        let signature =
            verify::parse_signature(&signature_text, &available.version, &package.asset.name)?;
        let mut verifier = verify::start(&self.key, &signature)?;
        let payload = dir.path().join(&package.asset.name);
        let mut file = OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&payload)
            .map_err(Error::io("create the download file"))?;
        let mut hasher = Sha256::new();
        let mut received = 0u64;
        let mut write_error = None;
        let total = package.asset.size;
        let counted = self.client.get(
            &package.asset.url,
            "application/octet-stream",
            total,
            &mut |chunk, _| {
                if let Err(error) = file.write_all(chunk) {
                    write_error = Some(error);
                    return ControlFlow::Break(());
                }
                verifier.update(chunk);
                hasher.update(chunk);
                received += chunk.len() as u64;
                progress(Progress { received, total })
            },
        );
        if let Some(source) = write_error {
            return Err(Error::Io {
                context: "write the download",
                source,
            });
        }
        let counted = counted?;
        if counted != total {
            return Err(Error::SizeMismatch {
                expected: total,
                received: counted,
            });
        }
        file.sync_all().map_err(Error::io("flush the download"))?;
        verify::finish(verifier)?;
        Ok(Verified {
            dir,
            payload,
            version: available.version.clone(),
            asset_name: package.asset.name.clone(),
            sha256: hasher.finalize().into(),
            kind: kind.clone(),
        })
    }

    /// Replaces this installation after the caller has saved documents and joined all codec/background work.
    /// On success the caller must quit
    /// promptly (see [`Relaunch`]).
    ///
    /// # Errors
    /// Returns an error if the payload changed, is not newer or compatible, cannot be installed,
    /// or the restart helper cannot start. Helper failure can occur after replacement; the
    /// caller should show the error and allow a manual restart.
    pub fn apply(&self, verified: Verified, installation: &mut InstallLock) -> Result<Relaunch> {
        self.ensure_newer(&verified.version)?;
        if verified.kind != self.platform.install {
            return Err(Error::ManualOnly);
        }
        verified.recheck()?;
        installation.begin_install(&self.platform)?;
        match verified.kind {
            #[cfg(unix)]
            InstallKind::MacosBundle { ref bundle } => apply::macos::apply(
                bundle,
                &verified.payload,
                verified.dir.path(),
                &verified.version,
            ),
            #[cfg(unix)]
            InstallKind::LinuxAppImage { ref path } => {
                apply::linux::apply(path, &verified.payload, &verified.version)
            }
            #[cfg(windows)]
            InstallKind::WindowsMsi { ref dir } => {
                let hash = verified.sha256_hex();
                apply::windows::apply_msi(
                    dir,
                    &verified.payload,
                    &hash,
                    verified.dir.keep(),
                    &verified.version,
                )
            }
            #[cfg(windows)]
            InstallKind::WindowsPortable { ref dir } => apply::windows::apply_portable(
                dir,
                &verified.payload,
                verified.dir.keep(),
                &verified.asset_name,
                &verified.version,
            ),
            _ => Err(Error::UnsupportedPlatform),
        }
    }

    fn ensure_newer(&self, offered: &Version) -> Result<()> {
        if version::is_offerable(&self.current, offered) {
            Ok(())
        } else {
            Err(Error::NotNewer {
                offered: offered.to_string(),
                current: self.current.to_string(),
            })
        }
    }
}

/// A private (0700) directory beside the installation, on the same filesystem as the final rename.
fn staging_dir(kind: &InstallKind) -> Result<tempfile::TempDir> {
    let parent = match kind {
        InstallKind::MacosBundle { bundle } => bundle.parent(),
        InstallKind::LinuxAppImage { path } => path.parent(),
        InstallKind::WindowsPortable { dir } => Some(dir.as_path()),
        _ => None,
    };
    let mut builder = tempfile::Builder::new();
    builder.prefix(STAGING_PREFIX);
    // The default is 0o777 minus the umask. Private to the user from the first moment.
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        builder.permissions(fs::Permissions::from_mode(0o700));
    }
    match parent {
        Some(parent) => builder
            .tempdir_in(parent)
            .map_err(|_| Error::ParentNotWritable(parent.to_path_buf())),
        None => builder
            .tempdir()
            .map_err(Error::io("create a temporary directory")),
    }
}

/// Removes only abandoned staging directories older than one day. Never removes rollback backups.
pub fn cleanup_leftovers(platform: &Platform) {
    use std::time::{Duration, SystemTime};
    let parent = match &platform.install {
        InstallKind::MacosBundle { bundle } => bundle.parent(),
        InstallKind::LinuxAppImage { path } => path.parent(),
        InstallKind::WindowsPortable { dir } => Some(dir.as_path()),
        _ => None,
    };
    let Some(parent) = parent else { return };
    let Ok(entries) = fs::read_dir(parent) else {
        return;
    };
    for entry in entries.flatten() {
        let stale = entry
            .file_name()
            .to_string_lossy()
            .starts_with(STAGING_PREFIX)
            && entry
                .file_type()
                .is_ok_and(|t| t.is_dir() && !t.is_symlink())
            && entry.metadata().and_then(|m| m.modified()).is_ok_and(|t| {
                SystemTime::now()
                    .duration_since(t)
                    .is_ok_and(|d| d > Duration::from_secs(86400))
            });
        if stale {
            let _ = fs::remove_dir_all(entry.path());
        }
    }
}
