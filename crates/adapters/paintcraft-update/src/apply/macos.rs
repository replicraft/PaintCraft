//! macOS: replace `PaintCraft.app` with the one from `paintcraft-<v>-macos-<arch>-update.zip`.
//!
//! Steps: preflight (where does the app run), extract next to the bundle (same volume), check the
//! new bundle (plist, executable, `codesign --verify`), swap by two renames with rollback, and
//! let a detached helper open the new bundle once this process has exited.

use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

use semver::Version;

use super::{Relaunch, random_suffix};
use crate::error::{Error, Result};
use crate::plist;
use crate::zipx::{self, Limits};

/// `CFBundleIdentifier` of `apps/paintcraft/resources/macos/Info.plist.in`.
pub const BUNDLE_ID: &str = "io.github.replicraft.PaintCraft";
/// `CFBundleExecutable` written by `scripts/packaging/package_macos.py`.
pub const EXECUTABLE: &str = "paintcraft-launcher";
/// The code identifier every program in the bundle is signed with (keychain identity).
pub const CODE_IDENTIFIER: &str = "io.github.replicraft.PaintCraft";

/// Refuses locations where an in-place swap cannot work or would be undone.
pub fn preflight(bundle: &Path) -> Result<()> {
    if bundle.to_string_lossy().contains("/AppTranslocation/") {
        return Err(Error::AppTranslocated);
    }
    let parent = bundle
        .parent()
        .ok_or_else(|| Error::BundleInvalid("bundle has no parent".into()))?;
    if rustix::fs::statvfs(parent)
        .is_ok_and(|s| s.f_flag.contains(rustix::fs::StatVfsMountFlags::RDONLY))
    {
        return Err(Error::ReadOnlyVolume(parent.to_path_buf()));
    }
    if rustix::fs::access(parent, rustix::fs::Access::WRITE_OK).is_err() {
        return Err(Error::ParentNotWritable(parent.to_path_buf()));
    }
    Ok(())
}

/// Extracts `zip` below `staging` and checks the new bundle. `codesign` is injectable so tests
/// need no signed app; production passes [`run_codesign`]. Returns the path of the new `.app`.
pub fn prepare(
    zip: &Path,
    staging: &Path,
    version: &Version,
    codesign: &dyn Fn(&Path) -> Result<()>,
) -> Result<PathBuf> {
    let target = staging.join("extracted");
    fs::create_dir(&target).map_err(Error::io("create the extraction directory"))?;
    let app = zipx::extract(zip, &target, "PaintCraft.app", &Limits::APP)?;
    check_bundle(&app, version)?;
    codesign(&app)?;
    Ok(app)
}

fn check_bundle(app: &Path, version: &Version) -> Result<()> {
    let bad = |why: &str| Error::BundleInvalid(why.to_owned());
    let plist_path = app.join("Contents/Info.plist");
    let meta = fs::symlink_metadata(&plist_path).map_err(|_| bad("no Info.plist"))?;
    if !meta.is_file() || meta.len() > 256 * 1024 {
        return Err(bad("Info.plist is not a plain small file"));
    }
    let xml = fs::read_to_string(&plist_path).map_err(|_| bad("Info.plist is not XML text"))?;
    let value = |key| plist::string_value(&xml, key);
    if value("CFBundleIdentifier").as_deref() != Some(BUNDLE_ID) {
        return Err(bad("unexpected bundle identifier"));
    }
    let numeric = format!("{}.{}.{}", version.major, version.minor, version.patch);
    if value("CFBundleShortVersionString").as_deref() != Some(numeric.as_str()) {
        return Err(bad("bundle version differs from the release version"));
    }
    if value("CFBundleExecutable").as_deref() != Some(EXECUTABLE) {
        return Err(bad("unexpected bundle executable"));
    }
    let exe = app.join("Contents/MacOS").join(EXECUTABLE);
    let meta = fs::symlink_metadata(&exe).map_err(|_| bad("bundle executable is missing"))?;
    if !meta.is_file() || !is_executable(&meta) {
        return Err(bad("bundle executable is not an executable file"));
    }

    let editor = app.join("Contents/Helpers/PaintCraft Editor.app/Contents");
    let editor_info = editor.join("Info.plist");
    let meta =
        fs::symlink_metadata(&editor_info).map_err(|_| bad("missing nested editor plist"))?;
    if !meta.is_file() || meta.len() > 256 * 1024 {
        return Err(bad("invalid editor plist"));
    }
    let xml = fs::read_to_string(editor_info).map_err(|_| bad("invalid editor plist text"))?;
    if plist::string_value(&xml, "CFBundleIdentifier").as_deref()
        != Some("io.github.replicraft.PaintCraft.editor")
        || plist::string_value(&xml, "CFBundleExecutable").as_deref() != Some("paintcraft")
        || plist::string_value(&xml, "CFBundleShortVersionString").as_deref()
            != Some(numeric.as_str())
    {
        return Err(bad("nested editor identity or version differs"));
    }
    let engine = fs::symlink_metadata(editor.join("MacOS/paintcraft"))
        .map_err(|_| bad("missing nested editor"))?;
    if !engine.is_file() || !is_executable(&engine) {
        return Err(bad("invalid nested editor"));
    }
    let target = if cfg!(target_arch = "aarch64") {
        "aarch64-apple-darwin"
    } else {
        "x86_64-apple-darwin"
    };
    crate::payload::codec_group(
        &editor.join("MacOS"),
        &editor.join("Resources"),
        version,
        target,
    )?;
    Ok(())
}

#[cfg(unix)]
fn is_executable(meta: &fs::Metadata) -> bool {
    use std::os::unix::fs::PermissionsExt;
    meta.permissions().mode() & 0o100 != 0
}

#[cfg(not(unix))]
fn is_executable(_: &fs::Metadata) -> bool {
    true
}

/// `codesign --verify --deep --strict` plus the code identifier the keychain identity relies on.
pub fn run_codesign(app: &Path) -> Result<()> {
    let run = |args: &[&str]| {
        Command::new("/usr/bin/codesign")
            .args(args)
            .arg(app)
            .output()
            .map_err(|e| Error::CodesignFailed(e.to_string()))
    };
    let verify = run(&["--verify", "--deep", "--strict"])?;
    if !verify.status.success() {
        return Err(Error::CodesignFailed(brief(&verify.stderr)));
    }
    let display = run(&["--display", "--verbose=2"])?;
    let text = String::from_utf8_lossy(&display.stderr);
    if !text
        .lines()
        .any(|l| l == format!("Identifier={CODE_IDENTIFIER}"))
    {
        return Err(Error::CodesignFailed(format!(
            "code identifier is not {CODE_IDENTIFIER}"
        )));
    }
    Ok(())
}

fn brief(bytes: &[u8]) -> String {
    String::from_utf8_lossy(bytes)
        .trim()
        .chars()
        .take(300)
        .collect()
}

/// Puts `new_app` where `bundle` is. The old bundle moves to `.<name>.old-<random>` first; if
/// the second rename fails the first is undone. Returns the path the old bundle now has.
pub fn swap(bundle: &Path, new_app: &Path) -> Result<PathBuf> {
    let parent = bundle
        .parent()
        .ok_or_else(|| Error::BundleInvalid("bundle has no parent".into()))?;
    let name = bundle
        .file_name()
        .ok_or_else(|| Error::BundleInvalid("bundle has no name".into()))?;
    let mut aside = std::ffi::OsString::from(".");
    aside.push(name);
    aside.push(format!(".old-{}", random_suffix()));
    let old = parent.join(aside);
    fs::rename(bundle, &old).map_err(Error::io("move the old app aside"))?;
    if let Err(error) = fs::rename(new_app, bundle) {
        // The old app is the only copy in place; put it back before reporting.
        fs::rename(&old, bundle)
            .map_err(Error::io("restore original app after failed replacement"))?;
        return Err(Error::Io {
            context: "move the new app into place",
            source: error,
        });
    }
    Ok(old)
}

/// Deletes `.<name>.old-*` siblings left by earlier updates (the helper removes the old bundle
/// after relaunch; this covers a helper that never ran).
pub fn cleanup_stale(bundle: &Path) {
    let (Some(parent), Some(name)) = (bundle.parent(), bundle.file_name()) else {
        return;
    };
    let prefix = format!(".{}.old-", name.to_string_lossy());
    let Ok(entries) = fs::read_dir(parent) else {
        return;
    };
    for entry in entries.flatten() {
        let file = entry.file_name();
        let file = file.to_string_lossy();
        let suffix = file.strip_prefix(&prefix);
        let is_ours =
            suffix.is_some_and(|s| s.len() == 32 && s.bytes().all(|b| b.is_ascii_hexdigit()));
        if is_ours
            && entry
                .file_type()
                .is_ok_and(|t| t.is_dir() && !t.is_symlink())
        {
            let _ = fs::remove_dir_all(entry.path());
        }
    }
}

/// The whole macOS step; `staging` is the verified download's private directory.
pub fn apply(bundle: &Path, zip: &Path, staging: &Path, version: &Version) -> Result<Relaunch> {
    preflight(bundle)?;
    let new_app = prepare(zip, staging, version, &run_codesign)?;
    let old = swap(bundle, &new_app)?;
    if let Err(error) = crate::helper::relaunch_open(std::process::id(), bundle, &old) {
        // Without a helper nothing would start the new app; keep the old one.
        fs::rename(bundle, &new_app)
            .and_then(|()| fs::rename(&old, bundle))
            .map_err(Error::io("restore original app after helper failure"))?;
        return Err(error);
    }
    Ok(Relaunch {
        version: version.clone(),
        launch: bundle.to_path_buf(),
    })
}
