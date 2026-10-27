//! Select one host installation layout before reading any manifest bytes.
use super::*;

pub(super) struct ManifestLayout {
    pub path: PathBuf,
    /// Bundle resources must not compete with an adjacent data manifest.
    pub forbidden_adjacent: Option<PathBuf>,
}
pub(super) fn exists(path: &Path) -> Result<bool> {
    match fs::symlink_metadata(path) {
        Ok(_) => Ok(true),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(false),
        Err(error) => Err(storage(error)),
    }
}
pub(super) fn resolve(directory: &Path, target: Target) -> Result<ManifestLayout> {
    let adjacent = directory.join(MANIFEST_NAME);
    let contents = directory.parent();
    let bundle = contents.and_then(Path::parent);
    let mac_bundle = matches!(target, Target::MacArm | Target::MacX64)
        && directory.file_name().is_some_and(|n| n == "MacOS")
        && contents.is_some_and(|p| p.file_name().is_some_and(|n| n == "Contents"))
        && bundle.is_some_and(|p| {
            p.extension().is_some_and(|n| n == "app")
                && p.file_stem().is_some_and(|n| !n.is_empty())
        });
    if mac_bundle {
        if exists(&adjacent)? {
            return Err(unavailable(
                "Mac application bundle has an adjacent helper manifest; Resources is required",
            ));
        }
        return Ok(ManifestLayout {
            path: contents.unwrap().join("Resources").join(MANIFEST_NAME),
            forbidden_adjacent: Some(adjacent),
        });
    }
    // Existing Linux prefix layout: adjacent wins when present. This is an
    // upfront location choice, never a retry after acquisition/integrity error.
    let path = if target.linux()
        && directory.file_name().is_some_and(|n| n == "bin")
        && !exists(&adjacent)?
    {
        directory
            .parent()
            .ok_or_else(|| unavailable("Linux helper installation prefix missing"))?
            .join("share/paintcraft")
            .join(MANIFEST_NAME)
    } else {
        adjacent
    };
    Ok(ManifestLayout {
        path,
        forbidden_adjacent: None,
    })
}
