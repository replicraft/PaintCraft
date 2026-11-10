//! Validate the signed helper group without executing any downloaded program.
use crate::{Error, Result};
use semver::Version;
use sha2::{Digest, Sha256};
use std::{fs, io::Read, path::Path};
pub(crate) fn codec_group(
    programs: &Path,
    resources: &Path,
    version: &Version,
    target: &str,
) -> Result<()> {
    let bad = || Error::BundleInvalid("codec helper group does not match the release".into());
    let manifest = resources.join("paintcraft-codec-worker.json");
    let meta = fs::symlink_metadata(&manifest).map_err(|_| bad())?;
    if !meta.is_file() || meta.len() > 65536 {
        return Err(bad());
    }
    let value: serde_json::Value =
        serde_json::from_slice(&fs::read(&manifest).map_err(|_| bad())?).map_err(|_| bad())?;
    if value.get("application_version").and_then(|v| v.as_str())
        != Some(version.to_string().as_str())
        || value.get("target").and_then(|v| v.as_str()) != Some(target)
    {
        return Err(bad());
    }
    let executable = if target.contains("windows") {
        "paintcraft-codec-worker.exe"
    } else {
        "paintcraft-codec-worker"
    };
    if value.get("executable").and_then(|v| v.as_str()) != Some(executable) {
        return Err(bad());
    }
    let path = programs.join(executable);
    let meta = fs::symlink_metadata(&path).map_err(|_| bad())?;
    if !meta.is_file() || meta.len() == 0 {
        return Err(bad());
    }
    let mut file = fs::File::open(path).map_err(|_| bad())?;
    let mut hash = Sha256::new();
    let mut buffer = [0u8; 65536];
    loop {
        let n = file.read(&mut buffer).map_err(|_| bad())?;
        if n == 0 {
            break;
        }
        hash.update(&buffer[..n]);
    }
    let hex: String = hash.finalize().iter().map(|n| format!("{n:02x}")).collect();
    if value.get("sha256").and_then(|v| v.as_str()) != Some(hex.as_str()) {
        return Err(bad());
    }
    Ok(())
}
