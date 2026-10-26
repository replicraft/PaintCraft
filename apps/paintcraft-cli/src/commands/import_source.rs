//! Same-descriptor prefix classification; streams keep the legacy snapshot policy.
use super::*;
use paintcraft_platform::source::SourceIdentity;
type Captured = (Vec<u8>, String, Option<SourceIdentity>);
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
    Ok(if &prefix == b"PK\x03\x04" {
        generic.min(paintcraft_formats::ora::CONVERSION_BYTES_MAX as u64)
    } else {
        generic
    })
}
pub(super) fn read_conversion_source(path: &Path, generic: u64, what: &str) -> Result<Captured> {
    let canonical_ora = path
        .extension()
        .is_some_and(|ext| ext.eq_ignore_ascii_case("ora"));
    if !path.metadata().map_err(|e| storage(what, &e))?.is_file() {
        return read_regular_or_bounded(
            path,
            if canonical_ora {
                generic.min(paintcraft_formats::ora::CONVERSION_BYTES_MAX as u64)
            } else {
                generic
            },
            what,
        );
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
}
