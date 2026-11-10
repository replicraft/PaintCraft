//! Every way an update can fail. Variants are specific enough for the UI to translate; the
//! `Display` text is English for logs only and never contains a URL query.

use std::path::PathBuf;

pub type Result<T> = std::result::Result<T, Error>;

#[derive(Debug, thiserror::Error)]
#[non_exhaustive]
pub enum Error {
    #[error(
        "another PaintCraft editor is using this installation; close other windows before updating"
    )]
    InstallationBusy,
    #[error(
        "another updater took the installation lease; this editor must exit without resuming work"
    )]
    InstallationLockLost,
    #[error("signed updates are not configured in this build")]
    SigningKeyUnconfigured,
    #[error("the running version {0:?} is not a semantic version")]
    BadCurrentVersion(String),
    #[error("this platform cannot be updated by PaintCraft")]
    UnsupportedPlatform,
    #[error("this build never replaces itself; open the release page instead")]
    ManualOnly,
    #[error("network error: {0}")]
    Network(String),
    #[error("refused host {0:?}: not a GitHub release host")]
    HostNotAllowed(String),
    #[error("refused a non-HTTPS URL")]
    InsecureUrl,
    #[error("too many redirects")]
    TooManyRedirects,
    #[error("server answered HTTP {0}")]
    HttpStatus(u16),
    #[error("response larger than the allowed {limit} bytes")]
    ResponseTooLarge { limit: u64 },
    #[error("size mismatch: the release lists {expected} bytes, {received} arrived")]
    SizeMismatch { expected: u64, received: u64 },
    #[error("the release data is malformed: {0}")]
    BadRelease(String),
    #[error("release tag {tag:?} does not match the asset version")]
    TagMismatch { tag: String },
    #[error("version {offered} is not newer than the running {current}")]
    NotNewer { offered: String, current: String },
    #[error("release has no asset {0:?} for this platform")]
    AssetMissing(String),
    #[error("release has no signature file {0:?}")]
    SignatureMissing(String),
    #[error("signature check failed: {0}")]
    SignatureInvalid(String),
    #[error("signed comment is {found:?}, expected {expected:?}")]
    TrustedCommentMismatch { expected: String, found: String },
    #[error("downloaded file changed after it was verified")]
    PayloadChanged,
    #[error("the download was cancelled")]
    Cancelled,
    #[error("the archive is unsafe: {0}")]
    UnsafeArchive(String),
    #[error("the archive cannot be read: {0}")]
    Archive(String),
    #[error("the update package is invalid: {0}")]
    BundleInvalid(String),
    #[error(
        "PaintCraft runs from a translocated (quarantined) copy; move it to Applications first"
    )]
    AppTranslocated,
    #[error("PaintCraft runs from a read-only location ({0}); drag it to Applications first")]
    ReadOnlyVolume(PathBuf),
    #[error("cannot write next to the installed app ({0})")]
    ParentNotWritable(PathBuf),
    #[error("code signature check of the new app failed: {0}")]
    CodesignFailed(String),
    #[error("cannot start the restart helper: {0}")]
    Helper(String),
    #[error("{context}: {source}")]
    Io {
        context: &'static str,
        #[source]
        source: std::io::Error,
    },
}

impl Error {
    pub(crate) fn io(context: &'static str) -> impl FnOnce(std::io::Error) -> Error {
        move |source| Error::Io { context, source }
    }
}
