//! Bounded, hostile-archive-safe zip extraction. Runs only on a payload whose signature already
//! verified, but is written as if it had not: a bug or a stolen signing key must not become a
//! write outside the target directory.

use std::collections::HashSet;
use std::fs::{self, File, OpenOptions};
use std::io::{Read, Write};
use std::path::{Path, PathBuf};

use crate::error::{Error, Result};

#[derive(Debug, Clone, Copy)]
pub struct Limits {
    pub max_entries: usize,
    /// Total bytes written, counted as they are written (the sizes the archive declares are not trusted).
    pub max_total: u64,
    /// An entry of at least `ratio_floor` bytes may not expand more than `max_ratio` times.
    pub max_ratio: u64,
    pub ratio_floor: u64,
}

impl Limits {
    /// A PaintCraft.app is about 100 MiB; the archive is at most 300 MiB.
    pub const APP: Limits = Limits {
        max_entries: 5_000,
        max_total: 1024 * 1024 * 1024,
        max_ratio: 100,
        ratio_floor: 1024 * 1024,
    };
}

fn unsafe_entry(name: &str, why: &str) -> Error {
    let shown: String = name.chars().filter(|c| !c.is_control()).take(120).collect();
    Error::UnsafeArchive(format!("{shown:?}: {why}"))
}

/// Splits an entry name into safe components or says why not.
fn components(name: &str) -> std::result::Result<Vec<&str>, &'static str> {
    if name.is_empty() || name.contains('\0') || name.contains('\\') {
        return Err("bad characters");
    }
    if name.starts_with('/') {
        return Err("absolute path");
    }
    let trimmed = name.strip_suffix('/').unwrap_or(name);
    let parts: Vec<&str> = trimmed.split('/').collect();
    for part in &parts {
        if part.is_empty() || *part == "." || *part == ".." {
            return Err("path escapes or is not normalised");
        }
        if part.contains([':', '<', '>', '"', '|', '?', '*'])
            || part.ends_with('.')
            || part.ends_with(' ')
        {
            return Err("name not portable");
        }
    }
    Ok(parts)
}

/// Extracts `archive` into the existing empty directory `dest`. Every entry must live below the
/// single top-level directory `top`; only regular files and directories are accepted. Returns the
/// path of `dest/top`.
pub fn extract(archive: &Path, dest: &Path, top: &str, limits: &Limits) -> Result<PathBuf> {
    let file = File::open(archive).map_err(Error::io("open the update archive"))?;
    let mut zip = zip::ZipArchive::new(file).map_err(|e| Error::Archive(e.to_string()))?;
    if zip.len() > limits.max_entries {
        return Err(Error::UnsafeArchive(format!("{} entries", zip.len())));
    }
    let mut seen = HashSet::new();
    let mut written_total = 0u64;
    let mut has_top = false;
    for index in 0..zip.len() {
        let mut entry = zip
            .by_index(index)
            .map_err(|e| Error::Archive(e.to_string()))?;
        let name = entry
            .name()
            .map_err(|_| {
                unsafe_entry(
                    &String::from_utf8_lossy(entry.name_raw()),
                    "name is not valid text",
                )
            })?
            .into_owned();
        if entry.encrypted() {
            return Err(unsafe_entry(&name, "encrypted"));
        }
        let parts = components(&name).map_err(|why| unsafe_entry(&name, why))?;
        if !top.is_empty() && parts[0] != top {
            return Err(unsafe_entry(
                &name,
                "outside the expected top-level directory",
            ));
        }
        has_top = true;
        if !seen.insert(parts.join("/")) {
            return Err(unsafe_entry(&name, "duplicate entry"));
        }
        let mode = entry.unix_mode();
        let kind = mode.map(|m| m & 0o170000);
        let is_dir = entry.is_dir();
        match kind {
            None | Some(0o100000) if !is_dir => {}
            None | Some(0o040000) if is_dir => {}
            _ => {
                return Err(unsafe_entry(
                    &name,
                    "not a plain file or directory (link or device)",
                ));
            }
        }
        let mut target = dest.to_path_buf();
        target.extend(&parts);
        if is_dir {
            fs::create_dir_all(&target).map_err(Error::io("create a directory"))?;
            set_mode(&target, 0o755)?;
            continue;
        }
        let declared = entry.size();
        let compressed = entry.compressed_size().max(1);
        if declared >= limits.ratio_floor && declared / compressed > limits.max_ratio {
            return Err(unsafe_entry(&name, "implausible compression ratio"));
        }
        if let Some(parent) = target.parent() {
            fs::create_dir_all(parent).map_err(Error::io("create a directory"))?;
        }
        // create_new: never overwrites and never follows a link that is already there.
        let mut out = OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&target)
            .map_err(Error::io("create an extracted file"))?;
        let allowed = declared.min(limits.max_total.saturating_sub(written_total));
        let copied = copy_capped(&mut entry, &mut out, allowed, &name)?;
        if copied != declared {
            return Err(unsafe_entry(&name, "size differs from the archive header"));
        }
        written_total += copied;
        drop(out);
        set_mode(&target, mode.map_or(0o644, |m| m & 0o777))?;
    }
    if !has_top {
        return Err(Error::Archive("empty archive".into()));
    }
    Ok(dest.join(top))
}

/// Copies at most `allowed` bytes; one more byte than that is an error.
fn copy_capped(from: &mut impl Read, to: &mut File, allowed: u64, name: &str) -> Result<u64> {
    let mut buffer = vec![0u8; 64 * 1024];
    let mut total = 0u64;
    loop {
        let n = from
            .read(&mut buffer)
            .map_err(|e| Error::Archive(e.to_string()))?;
        if n == 0 {
            return Ok(total);
        }
        total += n as u64;
        if total > allowed {
            return Err(unsafe_entry(name, "expands beyond the allowed size"));
        }
        to.write_all(&buffer[..n])
            .map_err(Error::io("write an extracted file"))?;
    }
}

/// Owner can always read and write; setuid, setgid, sticky and group/other write are dropped.
#[cfg(unix)]
fn set_mode(path: &Path, mode: u32) -> Result<()> {
    use std::os::unix::fs::PermissionsExt;
    let mode = (mode & 0o755) | 0o600 | if path.is_dir() { 0o100 } else { 0 };
    fs::set_permissions(path, fs::Permissions::from_mode(mode))
        .map_err(Error::io("set permissions"))
}

#[cfg(not(unix))]
fn set_mode(_path: &Path, _mode: u32) -> Result<()> {
    Ok(())
}
