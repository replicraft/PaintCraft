use super::*;
use std::{fs, path::PathBuf};
struct Fixture(PathBuf);
impl Fixture {
    fn new() -> Self {
        let path = std::env::temp_dir().join(format!(
            "paintcraft-source-capture-{}",
            uuid::Uuid::new_v4()
        ));
        fs::write(&path, b"captured source bytes").unwrap();
        Self(path)
    }
}
impl Drop for Fixture {
    fn drop(&mut self) {
        let _ = fs::remove_file(&self.0);
    }
}
#[test]
fn capture_and_recheck_use_acquired_descriptor_and_preserve_exact_bytes() {
    let f = Fixture::new();
    let source = SourceIdentity::existing_destination(&f.0).unwrap().unwrap();
    let bytes = source.capture_bytes(21, 64, &|| Ok(())).unwrap();
    assert_eq!(bytes, b"captured source bytes");
    let digest = Sha256::digest(&bytes).into();
    source.verify_bytes(&f.0, 21, &digest, &|| Ok(())).unwrap();
    fs::write(&f.0, b"mutated! source bytes").unwrap();
    assert_eq!(
        source
            .verify_bytes(&f.0, 21, &digest, &|| Ok(()))
            .unwrap_err()
            .code,
        ErrorCode::ConflictStaleBase
    );
}
#[test]
fn changed_namespace_never_retargets_capture_to_a_new_file() {
    let f = Fixture::new();
    let source = SourceIdentity::existing_destination(&f.0).unwrap().unwrap();
    let moved = f.0.with_extension("moved");
    fs::rename(&f.0, &moved).unwrap();
    fs::write(&f.0, b"different source data").unwrap();
    let bytes = source.capture_bytes(21, 64, &|| Ok(())).unwrap();
    assert_eq!(bytes, b"captured source bytes");
    assert!(
        source
            .verify_bytes(&f.0, 21, &Sha256::digest(&bytes).into(), &|| Ok(()))
            .is_err()
    );
    fs::remove_file(moved).unwrap();
}
#[test]
fn admitted_size_limits_growth_and_cancellation_precede_returned_bytes() {
    let f = Fixture::new();
    let source = SourceIdentity::existing_destination(&f.0).unwrap().unwrap();
    assert_eq!(
        source.capture_bytes(21, 20, &|| Ok(())).unwrap_err().code,
        ErrorCode::ResourceLimit
    );
    assert!(source.capture_bytes(20, 64, &|| Ok(())).is_err());
    assert_eq!(
        source
            .capture_bytes(21, 64, &|| Err(Error::new(
                ErrorCode::OperationCancelled,
                "cancel"
            )))
            .unwrap_err()
            .code,
        ErrorCode::OperationCancelled
    );
}

#[test]
fn cloned_identities_capture_and_verify_concurrently_without_shared_cursor_provenance() {
    let f = Fixture::new();
    let expected: std::sync::Arc<Vec<u8>> =
        std::sync::Arc::new((0..200_003).map(|i| (i * 7 + i / 31) as u8).collect());
    fs::write(&f.0, expected.as_slice()).unwrap();
    let source = SourceIdentity::existing_destination(&f.0).unwrap().unwrap();
    let digest: [u8; 32] = Sha256::digest(expected.as_slice()).into();
    let barrier = std::sync::Arc::new(std::sync::Barrier::new(8));
    let threads: Vec<_> = (0..8)
        .map(|_| {
            let source = source.clone();
            let expected = expected.clone();
            let barrier = barrier.clone();
            let path = f.0.clone();
            std::thread::spawn(move || {
                barrier.wait();
                for _ in 0..4 {
                    let bytes = source
                        .capture_bytes(expected.len() as u64, 1 << 20, &|| Ok(()))
                        .unwrap();
                    assert_eq!(bytes, *expected);
                    source
                        .verify_bytes(&path, expected.len() as u64, &digest, &|| Ok(()))
                        .unwrap();
                }
            })
        })
        .collect();
    for worker in threads {
        worker.join().unwrap();
    }
}
