//! Same-descriptor prefix classification; streams keep the legacy snapshot policy.
use super::*;
use paintcraft_platform::source::SourceIdentity;
type Captured = (Vec<u8>, String, Option<SourceIdentity>);
fn signature_limit(prefix: &[u8], generic: u64, canonical_ora: bool) -> u64 {
    if canonical_ora || prefix.starts_with(b"PK\x03\x04") {
        generic.min(paintcraft_formats::ora::CONVERSION_BYTES_MAX as u64)
    } else if prefix.starts_with(b"II\x2a\0")
        || prefix.starts_with(b"MM\0\x2a")
        || prefix.starts_with(b"II\x2b\0")
        || prefix.starts_with(b"MM\0\x2b")
    {
        generic.min(paintcraft_formats::tiff_pages::BYTES_MAX as u64)
    } else {
        generic
    }
}
fn selected_limit(file: &mut File, generic: u64, canonical_ora: bool) -> Result<u64> {
    if canonical_ora {
        return Ok(generic.min(paintcraft_formats::ora::CONVERSION_BYTES_MAX as u64));
    }
    let mut prefix = [0; 4];
    match file.read_exact(&mut prefix) {
        Ok(()) => {}
        Err(error) if error.kind() == std::io::ErrorKind::UnexpectedEof => return Ok(generic),
        Err(error) => return Err(storage("source signature", &error)),
    }
    Ok(signature_limit(&prefix, generic, canonical_ora))
}
pub(super) fn read_conversion_source(path: &Path, generic: u64, what: &str) -> Result<Captured> {
    let canonical_ora = path
        .extension()
        .is_some_and(|ext| ext.eq_ignore_ascii_case("ora"));
    let canonical_tiff = path
        .extension()
        .is_some_and(|ext| ext.eq_ignore_ascii_case("tif") || ext.eq_ignore_ascii_case("tiff"));
    let generic = if canonical_tiff {
        generic.min(paintcraft_formats::tiff_pages::BYTES_MAX as u64)
    } else {
        generic
    };
    if !path.metadata().map_err(|e| storage(what, &e))?.is_file() {
        let mut file = File::open(path).map_err(|e| storage(what, &e))?;
        let mut bytes = Vec::new();
        (&mut file)
            .take(4)
            .read_to_end(&mut bytes)
            .map_err(|e| storage(what, &e))?;
        let limit = signature_limit(&bytes, generic, canonical_ora);
        if bytes.len() as u64 > limit {
            return Err(Error::new(
                ErrorCode::ResourceLimit,
                "stream exceeds input allowance",
            ));
        }
        file.take(limit + 1 - bytes.len() as u64)
            .read_to_end(&mut bytes)
            .map_err(|e| storage(what, &e))?;
        if bytes.len() as u64 > limit {
            return Err(Error::new(
                ErrorCode::ResourceLimit,
                "stream exceeds input allowance",
            ));
        }
        let sha = hex(&Sha256::digest(&bytes));
        return Ok((bytes, sha, None));
    }
    let mut file = File::open(path).map_err(|e| storage(what, &e))?;
    let metadata = file.metadata().map_err(|e| storage(what, &e))?;
    if !metadata.is_file() || metadata.len() > generic {
        return Err(Error::new(
            ErrorCode::ResourceLimit,
            "source exceeds generic allowance",
        ));
    }
    let limit = selected_limit(&mut file, generic, canonical_ora)?;
    let identity = SourceIdentity::from_file(file)?;
    // Positional capture starts at zero without reopening the pathname.
    let bytes = identity.capture_bytes(metadata.len(), limit, &|| Ok(()))?;
    let sha = hex(&Sha256::digest(&bytes));
    Ok((bytes, sha, Some(identity)))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;
    struct Cleanup(PathBuf);
    impl Drop for Cleanup {
        fn drop(&mut self) {
            let _ = std::fs::remove_dir_all(&self.0);
        }
    }
    #[test]
    fn only_potential_ora_uses_the_smaller_bound_and_capture_keeps_the_same_file() {
        let root =
            std::env::temp_dir().join(format!("paintcraft-ora-acquire-{}", uuid::Uuid::new_v4()));
        std::fs::create_dir(&root).unwrap();
        let large = root.join("large");
        let mut file = File::create(&large).unwrap();
        file.write_all(b"PCND").unwrap();
        file.set_len(paintcraft_formats::ora::CONVERSION_BYTES_MAX as u64 + 1)
            .unwrap();
        drop(file);
        let mut file = File::open(&large).unwrap();
        assert_eq!(
            selected_limit(&mut file, INPUT_MAX, false).unwrap(),
            INPUT_MAX
        );
        let source = root.join("source");
        let bytes = include_bytes!(
            "../../../../crates/services/paintcraft-formats/tests/ora-group-fixtures/neutral.ora"
        );
        std::fs::write(&source, bytes).unwrap();
        let (actual, _, identity) = read_conversion_source(&source, INPUT_MAX, "source").unwrap();
        assert_eq!(actual, bytes);
        let moved = root.join("moved.ora");
        std::fs::rename(&source, &moved).unwrap();
        assert!(identity.unwrap().matches_path(&moved).unwrap());
        std::fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn tiff_signature_is_bounded_on_the_open_descriptor_before_source_allocation() {
        let root =
            std::env::temp_dir().join(format!("paintcraft-tiff-acquire-{}", uuid::Uuid::new_v4()));
        let _cleanup = Cleanup(root.clone());
        std::fs::create_dir(&root).unwrap();
        let path = root.join("source");
        for signature in [b"II\x2a\0", b"MM\0\x2a", b"II\x2b\0", b"MM\0\x2b"] {
            let mut file = File::create(&path).unwrap();
            file.write_all(signature).unwrap();
            file.set_len(paintcraft_formats::tiff_pages::BYTES_MAX as u64 + 1)
                .unwrap();
            drop(file);
            assert_eq!(
                read_conversion_source(&path, INPUT_MAX, "TIFF")
                    .unwrap_err()
                    .code,
                ErrorCode::ResourceLimit
            );
        }
        assert_eq!(signature_limit(b"PCND", INPUT_MAX, false), INPUT_MAX);
        assert_eq!(
            signature_limit(b"PK\x03\x04", INPUT_MAX, false),
            paintcraft_formats::ora::CONVERSION_BYTES_MAX as u64
        );
        std::fs::remove_dir_all(root).unwrap();
    }
}
