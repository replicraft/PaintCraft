use super::*;
use paintcraft_core::types::{Rgba, resources::SharedBudget};
use paintcraft_document::{POLICY, new_document};
use std::{cell::Cell, collections::BTreeMap};
struct Folder(PathBuf);
impl Folder {
    fn new() -> Self {
        let path =
            std::env::temp_dir().join(format!("paintcraft-recovery-admission-{}", Uuid::new_v4()));
        fs::create_dir(&path).unwrap();
        Self(path)
    }
}
impl Drop for Folder {
    fn drop(&mut self) {
        fs::remove_dir_all(&self.0).unwrap();
    }
}
fn profile() -> Document {
    crate::pcnd::read(include_bytes!(
        "../../tests/pcnd-metadata-fixtures/rgb-profile.pcnd"
    ))
    .unwrap()
}
fn plain() -> Document {
    new_document(2, 3, Some(Rgba([7, 8, 9, 255])), 1, POLICY).unwrap()
}
fn metadata(budget: &SharedBudget) -> crate::pcnd::MetadataAdmission<'_> {
    crate::pcnd::MetadataAdmission {
        bytes: paintcraft_document::source_metadata::BYTES_MAX,
        budget,
    }
}
fn files(dir: &Path) -> BTreeMap<PathBuf, Vec<u8>> {
    fs::read_dir(dir)
        .unwrap()
        .map(|e| {
            let p = e.unwrap().path();
            let b = fs::read(&p).unwrap();
            (p, b)
        })
        .collect()
}
fn orphan(dir: &Path, doc: &Document) -> Found {
    let owner = Recovery::start(dir).unwrap();
    owner.checkpoint_document(doc, None).unwrap();
    let session = owner.session();
    drop(owner);
    Recovery::scan(dir)
        .into_iter()
        .find(|f| f.session == session)
        .unwrap()
}
fn cleared(dir: &Path) -> Uuid {
    let owner = Recovery::start(dir).unwrap();
    owner.checkpoint_document(&plain(), None).unwrap();
    let id = owner.session();
    fs::write(tombstone(dir, id), cleared_bytes(id)).unwrap();
    drop(owner);
    id
}
fn scan(dir: &Path, budget: &SharedBudget, check: &dyn Fn() -> Result<()>) -> Result<ScanReport> {
    Recovery::scan_report_with_metadata_admission_checked(dir, metadata(budget), check)
}
#[test]
fn scan_reuses_one_caller_charge_and_denial_never_downgrades_or_reclaims() {
    let folder = Folder::new();
    let doc = profile();
    let charge = doc.source_metadata.as_ref().unwrap().storage_usage().1;
    orphan(&folder.0, &doc);
    orphan(&folder.0, &doc);
    let budget = SharedBudget::new(charge);
    assert_eq!(scan(&folder.0, &budget, &|| Ok(())).unwrap().found.len(), 2);
    assert_eq!(budget.usage().reserved, 0);
    assert_eq!(budget.usage().peak, charge);
    let owner = Recovery::start(&folder.0).unwrap();
    owner.checkpoint_document(&plain(), None).unwrap();
    owner.checkpoint_document(&doc, None).unwrap();
    drop(owner);
    let id = cleared(&folder.0);
    let before = files(&folder.0);
    let denied = SharedBudget::new(0);
    assert_eq!(
        scan(&folder.0, &denied, &|| Ok(())).unwrap_err().code,
        ErrorCode::ResourceLimit
    );
    assert_eq!(files(&folder.0), before);
    assert!(tombstone(&folder.0, id).exists());
    assert_eq!(denied.usage().reserved, 0);
    let report = scan(&folder.0, &budget, &|| Ok(())).unwrap();
    assert_eq!(report.found.len(), 3);
    assert!(!tombstone(&folder.0, id).exists());
}
#[test]
fn every_scan_callback_failure_preserves_all_files_before_cleanup_commit() {
    let folder = Folder::new();
    orphan(&folder.0, &profile());
    let budget = SharedBudget::new(1 << 20);
    let count = Cell::new(0);
    cleared(&folder.0);
    scan(&folder.0, &budget, &|| {
        count.set(count.get() + 1);
        Ok(())
    })
    .unwrap();
    for code in [ErrorCode::OperationCancelled, ErrorCode::CodecFailed] {
        for sticky in [false, true] {
            for stop in 1..=count.get() {
                let candidate = cleared(&folder.0);
                let before = files(&folder.0);
                let visits = Cell::new(0);
                let result = scan(&folder.0, &budget, &|| {
                    visits.set(visits.get() + 1);
                    if visits.get() == stop || sticky && visits.get() > stop {
                        Err(Error::new(code, "scan checkpoint sentinel"))
                    } else {
                        Ok(())
                    }
                });
                assert_eq!(result.unwrap_err().code, code);
                assert_eq!(files(&folder.0), before);
                assert_eq!(budget.usage().reserved, 0);
                // Explicit authorized cleanup between independent cancellation cases.
                Recovery::discard(&folder.0, candidate).unwrap();
            }
        }
    }
}
#[test]
fn claims_readers_and_byte_checkpoints_retain_the_exact_caller_realm() {
    let folder = Folder::new();
    let offer = orphan(&folder.0, &profile());
    let before = files(&folder.0);
    let denied = SharedBudget::new(0);
    assert_eq!(
        Recovery::claim_with_metadata_admission_checked(&offer, metadata(&denied), &|| Ok(()))
            .unwrap_err()
            .code,
        ErrorCode::ResourceLimit
    );
    assert_eq!(
        Recovery::read_checkpoint_with_metadata_admission_checked(
            &offer.checkpoint,
            metadata(&denied),
            &|| Ok(())
        )
        .unwrap_err()
        .code,
        ErrorCode::ResourceLimit
    );
    assert_eq!(files(&folder.0), before);
    let budget = SharedBudget::new(1 << 20);
    let calls = Cell::new(0);
    let claim = Recovery::claim_with_metadata_admission_checked(&offer, metadata(&budget), &|| {
        calls.set(calls.get() + 1);
        Ok(())
    })
    .unwrap();
    let source = claim.document.source_metadata.as_ref().unwrap().clone();
    let charge = source.storage_usage().1;
    assert_eq!(budget.usage().reserved, charge);
    drop(claim);
    assert_eq!(budget.usage().reserved, charge);
    drop(source);
    assert_eq!(budget.usage().reserved, 0);
    for stop in 1..=calls.get() {
        let visits = Cell::new(0);
        let result =
            Recovery::claim_with_metadata_admission_checked(&offer, metadata(&budget), &|| {
                visits.set(visits.get() + 1);
                if visits.get() == stop {
                    Err(Error::new(ErrorCode::OperationCancelled, "claim sentinel"))
                } else {
                    Ok(())
                }
            });
        assert_eq!(result.unwrap_err().code, ErrorCode::OperationCancelled);
        assert_eq!(budget.usage().reserved, 0);
        assert_eq!(files(&folder.0), before);
    }
    let owner = Recovery::start(&folder.0).unwrap();
    let snapshot = files(&folder.0);
    let bytes = fs::read(&offer.checkpoint).unwrap();
    assert_eq!(
        owner
            .checkpoint_with_metadata_admission_checked(&bytes, None, metadata(&denied), &|| Ok(()))
            .unwrap_err()
            .code,
        ErrorCode::ResourceLimit
    );
    assert_eq!(files(&folder.0), snapshot);
    owner
        .checkpoint_with_metadata_admission_checked(&bytes, None, metadata(&budget), &|| Ok(()))
        .unwrap();
    assert_eq!(budget.usage().reserved, 0);
}
#[test]
fn future_versions_are_retained_without_fallback_while_integrity_corruption_still_falls_back() {
    use sha2::{Digest, Sha256};
    let folder = Folder::new();
    let owner = Recovery::start(&folder.0).unwrap();
    let id = owner.session();
    owner.checkpoint_document(&plain(), None).unwrap();
    owner.checkpoint_document(&profile(), None).unwrap();
    drop(owner);
    let (mut commit, _) = reader::load_commit(&folder.0, id, 2).unwrap();
    let path = payload_path(&folder.0, id, commit.payload);
    let original = fs::read(&path).unwrap();
    let mut future = original.clone();
    future[4] = 7;
    let end = future.len() - 32;
    let digest = Sha256::digest(&future[..end]);
    future[end..].copy_from_slice(&digest);
    assert_eq!(crate::pcnd::probe(&future).unwrap().container_version, 7);
    assert_eq!(
        crate::pcnd::probe(&future)
            .unwrap()
            .require_supported()
            .unwrap_err()
            .code,
        ErrorCode::CapabilityUnsupported
    );
    fs::write(&path, &future).unwrap();
    commit.sha256 = hash(&future);
    let bytes = commit.encode();
    fs::write(commit_path(&folder.0, id, 2), &bytes).unwrap();
    fs::write(
        head_path(&folder.0, id, 0),
        Head {
            sequence: 2,
            commit_sha256: hash(&bytes),
        }
        .encode(),
    )
    .unwrap();
    let before = files(&folder.0);
    let budget = SharedBudget::new(1 << 20);
    let report = scan(&folder.0, &budget, &|| Ok(())).unwrap();
    assert!(report.found.is_empty());
    assert_eq!(report.unsupported_sessions, 1);
    assert_eq!(files(&folder.0), before);
    fs::write(&path, b"actually corrupt").unwrap();
    let report = scan(&folder.0, &budget, &|| Ok(())).unwrap();
    assert_eq!(report.found[0].sequence, 1);
    assert_eq!(report.unsupported_sessions, 0);
}
#[test]
fn deferred_cleanup_is_bounded_eventual_and_rechecks_namespace_before_deletion() {
    let folder = Folder::new();
    for _ in 0..66 {
        let id = Uuid::new_v4();
        fs::write(folder.0.join(format!("{id}.lock")), []).unwrap();
    }
    let budget = SharedBudget::new(0);
    scan(&folder.0, &budget, &|| Ok(())).unwrap();
    assert_eq!(fs::read_dir(&folder.0).unwrap().count(), 2);
    scan(&folder.0, &budget, &|| Ok(())).unwrap();
    assert_eq!(fs::read_dir(&folder.0).unwrap().count(), 0);
    let id = cleared(&folder.0);
    let mut batch = cleanup::Batch::default();
    let lock = OpenOptions::new()
        .read(true)
        .write(true)
        .open(folder.0.join(format!("{id}.lock")))
        .unwrap();
    lock.try_lock().unwrap();
    batch.defer(&folder.0, id, lock, true, &mut vec![]).unwrap();
    let path = folder.0.join(format!("{id}.lock"));
    let moved = folder.0.join("retained-original-lock");
    fs::rename(&path, &moved).unwrap();
    fs::write(&path, []).unwrap();
    let before = files(&folder.0);
    let mut issues = vec![];
    batch.commit(&mut issues);
    assert!(!issues.is_empty());
    assert_eq!(files(&folder.0), before);
}
