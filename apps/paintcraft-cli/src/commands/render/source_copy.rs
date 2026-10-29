//! A flattened render is a copy, including with --format overriding .pcnd.
use super::*;
use paintcraft_platform::source::SourceIdentity;
pub(super) struct SourceCopy {
    path: PathBuf,
    identity: SourceIdentity,
    bytes: u64,
    sha256: [u8; 32],
}
impl SourceCopy {
    pub(super) fn capture(path: &Path) -> Result<(Vec<u8>, String, Self)> {
        let (bytes, sha, identity) = read_captured(path, INPUT_MAX, "render source project")?;
        // Reuse the digest computed during acquisition instead of hashing a large
        // captured PCND buffer a second time merely to obtain its raw digest.
        let mut sha256 = [0; 32];
        if sha.len() != 64 {
            return Err(Error::new(
                ErrorCode::StorageFailed,
                "Captured source digest extent",
            ));
        }
        for (index, byte) in sha256.iter_mut().enumerate() {
            *byte = u8::from_str_radix(&sha[index * 2..index * 2 + 2], 16).map_err(|_| {
                Error::new(ErrorCode::StorageFailed, "Captured source digest encoding")
            })?;
        }
        let source = Self {
            path: path.to_owned(),
            identity,
            bytes: bytes.len() as u64,
            sha256,
        };
        Ok((bytes, sha, source))
    }
    fn protect(&self, destination: &Path) -> Result<()> {
        if self.identity.matches_path(destination)? {
            return Err(Error::new(
                ErrorCode::InputSchema,
                "Rendered copy must not replace its captured source project",
            ));
        }
        Ok(())
    }
    pub(super) fn check(&self, destination: &Path) -> Result<()> {
        self.protect(destination)?;
        self.identity
            .verify_bytes(&self.path, self.bytes, &self.sha256, &|| Ok(()))
    }
    pub(super) fn save(&self, destination: &Path, overwrite: bool, bytes: &[u8]) -> Outcome {
        self.save_with(destination, overwrite, bytes.len() as u64, |sink| {
            sink.write_all(bytes)
                .map_err(|error| storage("staged rendered copy", &error))
        })
    }
    fn save_with(
        &self,
        destination: &Path,
        overwrite: bool,
        limit: u64,
        encode: impl FnOnce(&mut dyn std::io::Write) -> Result<()>,
    ) -> Outcome {
        save::save_with_preparation(
            destination,
            overwrite,
            limit,
            |sink| {
                self.check(destination)?;
                encode(sink)
            },
            |outcome| {
                self.check(destination)?;
                if let Some(resolved) = outcome.resolved_destination()
                    && resolved != destination
                {
                    self.protect(resolved)?;
                }
                Ok(())
            },
        )
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn changed_acquired_source_refuses_before_staging_and_preserves_destination() {
        let dir =
            std::env::temp_dir().join(format!("paintcraft-render-source-{}", uuid::Uuid::new_v4()));
        std::fs::create_dir(&dir).unwrap();
        let source = dir.join("source.pcnd");
        let dest = dir.join("copy.png");
        std::fs::write(&source, b"original").unwrap();
        std::fs::write(&dest, b"destination").unwrap();
        let (_, _, captured) = SourceCopy::capture(&source).unwrap();
        assert_eq!(
            captured.check(&source).unwrap_err().code,
            ErrorCode::InputSchema
        );
        std::fs::write(&source, b"modified").unwrap();
        assert_eq!(
            captured.check(&dest).unwrap_err().code,
            ErrorCode::ConflictStaleBase
        );
        assert!(!captured.save(&dest, true, b"rendered").is_success());
        assert_eq!(std::fs::read(&source).unwrap(), b"modified");
        assert_eq!(std::fs::read(&dest).unwrap(), b"destination");
        assert_eq!(
            std::fs::read_dir(&dir).unwrap().count(),
            2,
            "staging files cleaned"
        );
        std::fs::remove_dir_all(dir).unwrap();
    }
}

#[cfg(test)]
#[test]
fn source_changed_during_staging_is_refused_at_the_real_prepublication_gate() {
    let dir = std::env::temp_dir().join(format!(
        "paintcraft-render-staged-source-{}",
        uuid::Uuid::new_v4()
    ));
    std::fs::create_dir(&dir).unwrap();
    let source = dir.join("source.pcnd");
    let dest = dir.join("copy.png");
    std::fs::write(&source, b"original").unwrap();
    std::fs::write(&dest, b"destination").unwrap();
    let (_, _, captured) = SourceCopy::capture(&source).unwrap();
    let outcome = captured.save_with(&dest, true, 8, |sink| {
        sink.write_all(b"rendered")
            .map_err(|error| storage("test staging", &error))?;
        std::fs::write(&source, b"modified").unwrap();
        Ok(())
    });
    assert!(!outcome.is_success());
    assert_eq!(std::fs::read(source).unwrap(), b"modified");
    assert_eq!(std::fs::read(dest).unwrap(), b"destination");
    assert_eq!(
        std::fs::read_dir(&dir).unwrap().count(),
        2,
        "staging files cleaned"
    );
    std::fs::remove_dir_all(dir).unwrap();
}
