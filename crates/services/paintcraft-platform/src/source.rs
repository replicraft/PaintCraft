//! Frozen acquired file identities shared by native resource consumers.
mod capture;
use paintcraft_core::types::{Error, ErrorCode, Result};
use std::{path::Path, sync::Arc};

/// Keep the actual acquired file open so its identity survives rename/unlink
/// without permitting inode reuse while a resource still owns it.
#[derive(Clone, Debug)]
pub struct SourceIdentity(Arc<same_file::Handle>);
impl SourceIdentity {
    pub fn from_file(file: std::fs::File) -> Result<Self> {
        same_file::Handle::from_file(file)
            .map(|handle| Self(Arc::new(handle)))
            .map_err(identity_error)
    }
    pub fn file(&self) -> &std::fs::File {
        self.0.as_file()
    }
    pub fn matches_path(&self, path: &Path) -> Result<bool> {
        Ok(Self::existing_destination(path)?.is_some_and(|candidate| self.matches(&candidate)))
    }
    pub fn matches(&self, other: &Self) -> bool {
        self.0 == other.0
    }
    pub fn existing_destination(path: &Path) -> Result<Option<Self>> {
        let metadata = match path.metadata() {
            Ok(metadata) => metadata,
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(None),
            Err(error) => return Err(identity_error(error)),
        };
        if !metadata.is_file() {
            return Ok(None);
        }
        let file = std::fs::File::open(path).map_err(identity_error)?;
        if !file.metadata().map_err(identity_error)?.is_file() {
            return Ok(None);
        }
        Self::from_file(file).map(Some)
    }
}
fn identity_error(error: std::io::Error) -> Error {
    Error::new(
        ErrorCode::StorageFailed,
        format!("Cannot inspect file identity: {}", error.kind()),
    )
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn clones_retain_the_acquired_file_after_rename_and_namespace_replacement() {
        use std::io::Read;
        let root =
            std::env::temp_dir().join(format!("paintcraft-identity-{}", uuid::Uuid::new_v4()));
        std::fs::create_dir(&root).unwrap();
        let original = root.join("original.abr");
        let moved = root.join("moved.abr");
        let alias = root.join("alias.png");
        std::fs::write(&original, b"acquired resource").unwrap();
        let source = SourceIdentity::from_file(std::fs::File::open(&original).unwrap()).unwrap();
        let retained = source.clone();
        drop(source);
        std::fs::rename(&original, &moved).unwrap();
        std::fs::hard_link(&moved, &alias).unwrap();
        std::fs::write(&original, b"namespace replacement").unwrap();
        assert!(retained.matches_path(&alias).unwrap());
        assert!(!retained.matches_path(&original).unwrap());
        let mut bytes = Vec::new();
        retained.file().read_to_end(&mut bytes).unwrap();
        assert_eq!(bytes, b"acquired resource");
        drop(retained);
        std::fs::remove_dir_all(root).unwrap();
    }
    #[cfg(unix)]
    #[test]
    fn ordinary_fifo_is_rejected_without_opening_it() {
        let path = std::env::temp_dir().join(format!("paintcraft-fifo-{}", uuid::Uuid::new_v4()));
        assert!(
            std::process::Command::new("mkfifo")
                .arg(&path)
                .status()
                .unwrap()
                .success()
        );
        let (send, receive) = std::sync::mpsc::channel();
        let input = path.clone();
        let worker = std::thread::spawn(move || {
            send.send(SourceIdentity::existing_destination(&input))
                .unwrap()
        });
        let result = receive
            .recv_timeout(std::time::Duration::from_secs(2))
            .expect("FIFO identity check must not block on open");
        assert!(result.unwrap().is_none());
        worker.join().unwrap();
        std::fs::remove_file(path).unwrap();
    }
    #[test]
    fn missing_and_directory_destinations_have_no_regular_file_identity() {
        assert!(
            SourceIdentity::existing_destination(&std::env::temp_dir())
                .unwrap()
                .is_none()
        );
        let missing =
            std::env::temp_dir().join(format!("paintcraft-missing-{}", uuid::Uuid::new_v4()));
        assert!(
            SourceIdentity::existing_destination(&missing)
                .unwrap()
                .is_none()
        );
    }
}
