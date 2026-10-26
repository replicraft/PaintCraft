//! Parent-owned seekable backing for codecs whose public API requires a path.
//! Keep this owner inside the admitted job until the child is reaped. Drop is
//! best effort; close reports cleanup failure. Parent death can leave an orphan.
use super::*;
use std::{fs, io::Write, path::PathBuf};
#[derive(Debug)]
pub struct PrivateSource {
    directory: PathBuf,
    path: PathBuf,
    identity: Option<SourceIdentity>,
    created: bool,
    cleaned: bool,
}
impl PrivateSource {
    /// A random directory and create-new file, with 0700/0600 modes on Unix.
    /// This is not a Windows ACL guarantee, filesystem sandbox or disk quota.
    pub fn create(
        bytes: &[u8],
        extension: &str,
        limit: u64,
        check: &dyn Fn() -> Result<()>,
    ) -> Result<Self> {
        check()?;
        if bytes.len() as u64 > limit {
            return Err(Error::new(
                ErrorCode::ResourceLimit,
                "Private codec source exceeds allowance",
            ));
        }
        if bytes.is_empty()
            || extension.is_empty()
            || extension.len() > 8
            || !extension.bytes().all(|value| value.is_ascii_alphanumeric())
        {
            return Err(Error::new(
                ErrorCode::InputSchema,
                "Invalid private codec source allowance or extension",
            ));
        }
        let root = std::env::temp_dir()
            .canonicalize()
            .map_err(identity_error)?;
        Self::create_in(bytes, extension, &root, check)
    }
    fn create_in(
        bytes: &[u8],
        extension: &str,
        root: &Path,
        check: &dyn Fn() -> Result<()>,
    ) -> Result<Self> {
        let directory = root.join(format!("paintcraft-codec-{}", uuid::Uuid::new_v4()));
        let mut builder = fs::DirBuilder::new();
        #[cfg(unix)]
        {
            use std::os::unix::fs::DirBuilderExt;
            builder.mode(0o700);
        }
        builder.create(&directory).map_err(identity_error)?;
        let path = directory.join(format!("source.{extension}"));
        let mut owned = Self {
            directory,
            path,
            identity: None,
            created: false,
            cleaned: false,
        };
        let mut options = fs::OpenOptions::new();
        options.read(true).write(true).create_new(true);
        #[cfg(unix)]
        {
            use std::os::unix::fs::OpenOptionsExt;
            options.mode(0o600);
        }
        let file = options.open(&owned.path).map_err(identity_error)?;
        // Capture the actual create-new file BEFORE any callbacks or writes.
        // On identity acquisition failure, never unlink an unverified pathname.
        owned.identity = Some(SourceIdentity::from_file(file)?);
        owned.created = true;
        let mut file = owned.identity().file();
        for chunk in bytes.chunks(65536) {
            check()?;
            file.write_all(chunk).map_err(identity_error)?;
        }
        file.flush().map_err(identity_error)?;
        check()?;
        Ok(owned)
    }
    pub fn path(&self) -> &Path {
        &self.path
    }
    /// The identity comes from the same create-new file, never a later lookup.
    pub fn identity(&self) -> &SourceIdentity {
        self.identity.as_ref().expect("complete private source")
    }
    fn cleanup(&mut self) -> Result<()> {
        if self.cleaned {
            return Ok(());
        }
        if self.created {
            let identity = self.identity.as_ref().expect("created file identity");
            let candidate = SourceIdentity::existing_destination(&self.path)?;
            if candidate
                .as_ref()
                .is_some_and(|candidate| !identity.matches(candidate))
            {
                return Err(Error::new(
                    ErrorCode::ConflictStaleBase,
                    "Private codec source path was replaced",
                ));
            }
            // Stop retrying pathname deletion after closing the verified handle.
            // If deletion fails, report it and leave the private file in place.
            self.created = false;
            self.identity.take();
            match fs::remove_file(&self.path) {
                Ok(()) => {}
                Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
                Err(error) => return Err(identity_error(error)),
            }
            self.created = false;
        }
        match fs::remove_dir(&self.directory) {
            Ok(()) => {
                self.cleaned = true;
                Ok(())
            }
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
                self.cleaned = true;
                Ok(())
            }
            Err(error) => Err(identity_error(error)),
        }
    }
    /// Call only after supervised child reap. No recursive deletion is used.
    pub fn close(mut self) -> Result<()> {
        self.cleanup()
    }
}
impl Drop for PrivateSource {
    fn drop(&mut self) {
        let _ = self.cleanup();
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn private_source_retains_exact_created_identity_and_closes_without_recursive_cleanup() {
        let source = PrivateSource::create(b"captured", "jxr", 32, &|| Ok(())).unwrap();
        let path = source.path().to_path_buf();
        let directory = path.parent().unwrap().to_path_buf();
        assert!(source.identity().matches_path(&path).unwrap());
        assert_eq!(fs::read(&path).unwrap(), b"captured");
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            assert_eq!(
                fs::metadata(&directory).unwrap().permissions().mode() & 0o777,
                0o700
            );
            assert_eq!(
                fs::metadata(&path).unwrap().permissions().mode() & 0o777,
                0o600
            );
        }
        source.close().unwrap();
        assert!(!directory.exists());
        assert!(PrivateSource::create(b"x", "../jxr", 32, &|| Ok(())).is_err());
        assert!(PrivateSource::create(b"xx", "jxr", 1, &|| Ok(())).is_err());
    }
    #[test]
    fn cancellation_drops_created_files_and_foreign_children_are_never_deleted() {
        use std::cell::Cell;
        let calls = Cell::new(0);
        let source = PrivateSource::create(&vec![7; 65537], "jxr", 65537, &|| {
            calls.set(calls.get() + 1);
            Ok(())
        })
        .unwrap();
        let directory = source.path().parent().unwrap().to_path_buf();
        fs::write(directory.join("foreign"), b"keep").unwrap();
        assert!(source.close().is_err());
        assert_eq!(fs::read(directory.join("foreign")).unwrap(), b"keep");
        fs::remove_file(directory.join("foreign")).unwrap();
        fs::remove_dir(directory).unwrap();
        for stop in 1..=calls.get() {
            let seen = Cell::new(0);
            let error = PrivateSource::create(&vec![7; 65537], "jxr", 65537, &|| {
                seen.set(seen.get() + 1);
                if seen.get() == stop {
                    Err(Error::new(ErrorCode::OperationCancelled, "stop"))
                } else {
                    Ok(())
                }
            })
            .unwrap_err();
            assert_eq!(error.code, ErrorCode::OperationCancelled);
        }
    }
    #[cfg(unix)]
    #[test]
    fn replacement_during_creation_cancellation_is_preserved() {
        use std::cell::Cell;
        let root =
            std::env::temp_dir().join(format!("paintcraft-private-test-{}", uuid::Uuid::new_v4()));
        fs::create_dir(&root).unwrap();
        let replacement = std::cell::RefCell::new(None);
        let visits = Cell::new(0);
        let error = PrivateSource::create_in(&vec![97; 65537], "jxr", &root, &|| {
            visits.set(visits.get() + 1);
            if visits.get() == 2 {
                let directory = fs::read_dir(&root).unwrap().next().unwrap().unwrap().path();
                let path = directory.join("source.jxr");
                fs::rename(&path, directory.join("original-held.jxr")).unwrap();
                fs::write(&path, b"replacement must survive").unwrap();
                *replacement.borrow_mut() = Some(path);
                return Err(Error::new(
                    ErrorCode::OperationCancelled,
                    "cancel after namespace replacement",
                ));
            }
            Ok(())
        })
        .unwrap_err();
        assert_eq!(error.code, ErrorCode::OperationCancelled);
        let path = replacement.into_inner().unwrap();
        assert_eq!(fs::read(&path).unwrap(), b"replacement must survive");
        assert_eq!(
            fs::read(path.parent().unwrap().join("original-held.jxr")).unwrap(),
            vec![97; 65536]
        );
        fs::remove_file(path.parent().unwrap().join("original-held.jxr")).unwrap();
        fs::remove_file(&path).unwrap();
        fs::remove_dir(path.parent().unwrap()).unwrap();
        fs::remove_dir(root).unwrap();
    }
}
