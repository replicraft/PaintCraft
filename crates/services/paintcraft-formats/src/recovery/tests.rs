use super::reader::load_commit;
use super::*;
use paintcraft_core::types::Rgba;
use paintcraft_document::{POLICY, new_document};
fn scratch() -> PathBuf {
    let dir = std::env::temp_dir().join(format!("paintcraft-recovery-journal-{}", Uuid::new_v4()));
    fs::create_dir(&dir).unwrap();
    dir
}
fn document(seed: u64) -> Document {
    new_document(8, 8, Some(Rgba([seed as u8, 7, 9, 255])), seed, POLICY).unwrap()
}
fn only(dir: &Path) -> Found {
    let report = Recovery::scan_report(dir);
    assert_eq!(report.found.len(), 1, "{:?}", report.issues);
    report.found.into_iter().next().unwrap()
}
#[test]
fn live_lock_hides_checkpoint_and_discard_cannot_delete_a_live_session() {
    let dir = scratch();
    let recovery = Recovery::start(&dir).unwrap();
    let doc = document(1);
    recovery
        .checkpoint_document(&doc, Some(Path::new("original.pcnd")))
        .unwrap();
    assert!(Recovery::scan(&dir).is_empty());
    assert!(Recovery::discard(&dir, recovery.session()).is_err());
    drop(recovery);
    let found = only(&dir);
    assert_eq!(found.original.as_deref(), Some("original.pcnd"));
    assert!(
        Recovery::read_checkpoint(&found.checkpoint)
            .unwrap()
            .same_semantics(&doc)
    );
    Recovery::discard(&dir, found.session).unwrap();
    assert_eq!(fs::read_dir(&dir).unwrap().count(), 0);
    fs::remove_dir_all(dir).unwrap();
}
#[test]
fn durable_heads_keep_two_snapshots_and_gc_removes_only_obsolete_payloads() {
    let dir = scratch();
    let recovery = Recovery::start(&dir).unwrap();
    for seed in 1..=10 {
        let published = recovery.checkpoint_document(&document(seed), None).unwrap();
        assert_eq!(published.sequence, seed);
        assert_eq!(published.state_id, document(seed).state_id);
    }
    let session = recovery.session();
    assert_eq!(
        fs::read_dir(&dir).unwrap().count(),
        7,
        "lock, two heads, two segments and two payloads"
    );
    assert!(!commit_path(&dir, session, 8).exists());
    assert!(commit_path(&dir, session, 9).exists());
    drop(recovery);
    let found = only(&dir);
    assert_eq!(found.sequence, 10);
    assert!(
        Recovery::read_checkpoint(&found.checkpoint)
            .unwrap()
            .same_semantics(&document(10))
    );
    fs::remove_dir_all(dir).unwrap();
}
#[test]
fn failures_at_protocol_boundaries_preserve_a_fully_backed_prefix() {
    for phase in [
        Phase::BeforePayload,
        Phase::AfterPayload,
        Phase::AfterCommit,
        Phase::AfterHead,
        Phase::BeforeGc,
    ] {
        let dir = scratch();
        let recovery = Recovery::start(&dir).unwrap();
        recovery.checkpoint_document(&document(1), None).unwrap();
        let session = recovery.session();
        let error = recovery
            .checkpoint_impl(&document(2), None, |at| {
                if at == Phase::AfterPayload {
                    assert!(
                        !commit_path(&dir, session, 2).exists(),
                        "payload must precede commit"
                    );
                }
                if at == Phase::AfterCommit {
                    let (record, _) = load_commit(&dir, session, 2).unwrap();
                    record.validate_payload(&dir, session).unwrap();
                }
                if at == phase {
                    Err(Error::new(
                        ErrorCode::StorageFailed,
                        "injected disk-full/barrier failure",
                    ))
                } else {
                    Ok(())
                }
            })
            .unwrap_err();
        assert_eq!(error.code, ErrorCode::StorageFailed);
        drop(recovery);
        let found = only(&dir);
        let expected = if matches!(phase, Phase::BeforePayload | Phase::AfterPayload) {
            1
        } else {
            2
        };
        assert_eq!(found.sequence, expected);
        assert!(
            Recovery::read_checkpoint(&found.checkpoint)
                .unwrap()
                .same_semantics(&document(expected))
        );
        fs::remove_dir_all(dir).unwrap();
    }
}
#[test]
fn gc_failure_does_not_deny_an_already_durable_checkpoint() {
    let dir = scratch();
    let recovery = Recovery::start(&dir).unwrap();
    for seed in [1, 2] {
        recovery.checkpoint_document(&document(seed), None).unwrap();
    }
    let obsolete = commit_path(&dir, recovery.session(), 1);
    fs::remove_file(&obsolete).unwrap();
    fs::create_dir(&obsolete).unwrap(); // injected failure reclaiming obsolete metadata
    let result = recovery.checkpoint_document(&document(3), None).unwrap();
    assert_eq!(result.sequence, 3);
    assert!(!result.warnings.is_empty());
    drop(recovery);
    assert_eq!(only(&dir).sequence, 3);
    fs::remove_dir_all(dir).unwrap();
}

#[test]
fn corrupt_latest_payload_falls_back_to_previous_durable_snapshot() {
    let dir = scratch();
    let recovery = Recovery::start(&dir).unwrap();
    for seed in 1..=3 {
        recovery.checkpoint_document(&document(seed), None).unwrap();
    }
    let session = recovery.session();
    let (latest, _) = load_commit(&dir, session, 3).unwrap();
    fs::write(payload_path(&dir, session, latest.payload), b"corrupt").unwrap();
    drop(recovery);
    let report = Recovery::scan_report(&dir);
    assert!(!report.issues.is_empty());
    assert_eq!(report.found[0].sequence, 2);
    assert!(
        Recovery::read_checkpoint(&report.found[0].checkpoint)
            .unwrap()
            .same_semantics(&document(2))
    );
    fs::remove_dir_all(dir).unwrap();
}
#[test]
fn torn_tail_stops_at_the_gap_and_never_skips_to_a_later_commit() {
    let dir = scratch();
    let recovery = Recovery::start(&dir).unwrap();
    recovery.checkpoint_document(&document(1), None).unwrap();
    for seed in [2, 3] {
        assert!(
            recovery
                .checkpoint_impl(&document(seed), None, |at| {
                    if at == Phase::AfterCommit {
                        Err(Error::new(
                            ErrorCode::StorageFailed,
                            "crash before head publication",
                        ))
                    } else {
                        Ok(())
                    }
                })
                .is_err()
        );
    }
    let path = commit_path(&dir, recovery.session(), 2);
    let bytes = fs::read(&path).unwrap();
    fs::write(&path, &bytes[..bytes.len() / 2]).unwrap();
    drop(recovery);
    let report = Recovery::scan_report(&dir);
    assert_eq!(report.found[0].sequence, 1);
    assert!(
        report
            .issues
            .iter()
            .any(|issue| issue.contains("stopped at journal 2"))
    );
    fs::remove_dir_all(dir).unwrap();
}
#[test]
fn missing_payload_is_a_gap_even_when_a_later_backed_record_exists() {
    let dir = scratch();
    let recovery = Recovery::start(&dir).unwrap();
    recovery.checkpoint_document(&document(1), None).unwrap();
    for seed in [2, 3] {
        let _ = recovery.checkpoint_impl(&document(seed), None, |at| {
            if at == Phase::AfterCommit {
                Err(Error::new(ErrorCode::StorageFailed, "stop"))
            } else {
                Ok(())
            }
        });
    }
    let (record, _) = load_commit(&dir, recovery.session(), 2).unwrap();
    fs::remove_file(payload_path(&dir, recovery.session(), record.payload)).unwrap();
    drop(recovery);
    assert_eq!(only(&dir).sequence, 1);
    fs::remove_dir_all(dir).unwrap();
}
#[test]
fn cleared_store_cannot_be_resurrected_by_files_left_from_interrupted_cleanup() {
    let dir = scratch();
    let recovery = Recovery::start(&dir).unwrap();
    recovery.checkpoint_document(&document(1), None).unwrap();
    let files: Vec<_> = owned_paths(&dir, recovery.session())
        .unwrap()
        .into_iter()
        .filter(|p| p.extension().is_none_or(|e| e != "lock"))
        .map(|p| {
            let bytes = fs::read(&p).unwrap();
            (p, bytes)
        })
        .collect();
    recovery.clear().unwrap();
    for (path, bytes) in files {
        fs::write(path, bytes).unwrap();
    }
    drop(recovery);
    assert!(
        Recovery::scan(&dir).is_empty(),
        "durable discard marker takes precedence"
    );
    assert_eq!(
        fs::read_dir(&dir).unwrap().count(),
        0,
        "interrupted discard is finished while holding orphan lock"
    );
    fs::remove_dir_all(dir).unwrap();
}
#[test]
fn clear_then_new_edits_start_a_new_journal_episode() {
    let dir = scratch();
    let recovery = Recovery::start(&dir).unwrap();
    recovery.checkpoint_document(&document(1), None).unwrap();
    recovery.clear().unwrap();
    assert_eq!(
        recovery
            .checkpoint_document(&document(2), None)
            .unwrap()
            .sequence,
        1
    );
    drop(recovery);
    let found = only(&dir);
    assert!(
        Recovery::read_checkpoint(&found.checkpoint)
            .unwrap()
            .same_semantics(&document(2))
    );
    fs::remove_dir_all(dir).unwrap();
}
#[test]
fn valid_legacy_checkpoints_still_recover_and_malformed_ones_are_retained() {
    let dir = scratch();
    let recovery = Recovery::start(&dir).unwrap();
    let session = recovery.session();
    let path = dir.join(format!("{session}.pcnd"));
    fs::write(&path, crate::pcnd::write(&document(7)).unwrap()).unwrap();
    fs::write(
        dir.join(format!("{session}.json")),
        b"{\"original\":\"legacy.pcnd\"}",
    )
    .unwrap();
    drop(recovery);
    let found = only(&dir);
    assert_eq!(found.sequence, 0);
    assert_eq!(found.original.as_deref(), Some("legacy.pcnd"));
    fs::write(&path, b"malformed").unwrap();
    let report = Recovery::scan_report(&dir);
    assert!(report.found.is_empty());
    assert!(!report.issues.is_empty());
    assert!(path.exists());
    fs::remove_dir_all(dir).unwrap();
}
#[test]
fn unrecognized_files_and_other_sessions_survive_discard() {
    let dir = scratch();
    let a = Recovery::start(&dir).unwrap();
    let b = Recovery::start(&dir).unwrap();
    a.checkpoint_document(&document(1), None).unwrap();
    b.checkpoint_document(&document(2), None).unwrap();
    let a_id = a.session();
    fs::write(dir.join("notes.txt"), b"keep").unwrap();
    fs::write(dir.join(format!("{a_id}.payload-not-a-uuid.pcnd")), b"keep").unwrap();
    let staging = dir.join(format!(
        ".{a_id}.payload-{}.pcnd.paintcraft-{}.tmp",
        Uuid::new_v4(),
        Uuid::new_v4().simple()
    ));
    fs::write(&staging, b"retained ambiguous staging").unwrap();
    drop(a);
    Recovery::discard(&dir, a_id).unwrap();
    assert!(
        !staging.exists(),
        "authorized staging is included in explicit discard"
    );
    assert!(dir.join("notes.txt").exists());
    assert!(dir.join(format!("{a_id}.payload-not-a-uuid.pcnd")).exists());
    drop(b);
    assert_eq!(Recovery::scan(&dir).len(), 1);
    fs::remove_dir_all(dir).unwrap();
}
#[test]
fn missing_directory_and_empty_orphan_locks_are_handled_without_inventing_recovery() {
    let dir = scratch();
    let id = Uuid::new_v4();
    fs::write(dir.join(format!("{id}.lock")), b"").unwrap();
    fs::write(dir.join("not-a-uuid.lock"), b"").unwrap();
    assert!(Recovery::scan(&dir).is_empty());
    assert!(!dir.join(format!("{id}.lock")).exists());
    assert!(dir.join("not-a-uuid.lock").exists());
    assert!(Recovery::scan(&dir.join("missing")).is_empty());
    fs::remove_dir_all(dir).unwrap();
}

#[cfg(unix)]
#[test]
fn recovery_payloads_cannot_follow_symlinks_outside_the_store() {
    use std::os::unix::fs::symlink;
    let dir = scratch();
    let outside = scratch();
    let recovery = Recovery::start(&dir).unwrap();
    recovery.checkpoint_document(&document(1), None).unwrap();
    let session = recovery.session();
    let (record, _) = load_commit(&dir, session, 1).unwrap();
    let payload = payload_path(&dir, session, record.payload);
    let target = outside.join("unrelated.pcnd");
    fs::copy(&payload, &target).unwrap();
    let original = fs::read(&target).unwrap();
    fs::remove_file(&payload).unwrap();
    symlink(&target, &payload).unwrap();
    drop(recovery);
    let report = Recovery::scan_report(&dir);
    assert!(report.found.is_empty());
    assert!(!report.issues.is_empty());
    Recovery::discard(&dir, session).unwrap();
    assert_eq!(
        fs::read(&target).unwrap(),
        original,
        "discard removes only the owned symlink"
    );
    fs::remove_dir_all(dir).unwrap();
    fs::remove_dir_all(outside).unwrap();
}

#[test]
fn publication_fences_reject_old_jobs_and_cross_owner_tokens() {
    let dir = scratch();
    let recovery = Recovery::start(&dir).unwrap();
    let other = Recovery::start(&dir).unwrap();
    let old = recovery.token();
    recovery
        .checkpoint_document_at(old, &document(1), None)
        .unwrap();
    let current = recovery.invalidate().unwrap();
    recovery.clear_at(current).unwrap();
    assert_eq!(
        recovery
            .checkpoint_document_at(old, &document(2), None)
            .unwrap_err()
            .code,
        ErrorCode::OperationCancelled
    );
    assert_eq!(
        other
            .checkpoint_document_at(current, &document(2), None)
            .unwrap_err()
            .code,
        ErrorCode::OperationCancelled
    );
    recovery
        .checkpoint_document_at(current, &document(3), None)
        .unwrap();
    assert_eq!(
        recovery.clear_at(old).unwrap_err().code,
        ErrorCode::OperationCancelled
    );
    drop(recovery);
    assert!(
        Recovery::read_checkpoint(&only(&dir).checkpoint)
            .unwrap()
            .same_semantics(&document(3))
    );
    drop(other);
    fs::remove_dir_all(dir).unwrap();
}

#[test]
fn invalidation_during_encoding_stops_old_publication_before_the_head() {
    for phase in [
        Phase::BeforePayload,
        Phase::AfterPayload,
        Phase::AfterCommit,
    ] {
        let dir = scratch();
        let recovery = Recovery::start(&dir).unwrap();
        recovery.checkpoint_document(&document(1), None).unwrap();
        let old = recovery.token();
        let result = recovery.checkpoint_at_impl(old, &document(2), None, |at| {
            if at == phase {
                recovery.invalidate().unwrap();
            }
            Ok(())
        });
        assert_eq!(result.unwrap_err().code, ErrorCode::OperationCancelled);
        recovery.clear_at(recovery.token()).unwrap();
        drop(recovery);
        assert!(
            Recovery::scan(&dir).is_empty(),
            "late work cannot resurrect cleared data"
        );
        fs::remove_dir_all(dir).unwrap();
    }
}

#[test]
fn a_claim_keeps_the_source_locked_until_handoff_or_cancellation() {
    let dir = scratch();
    let owner = Recovery::start(&dir).unwrap();
    owner.checkpoint_document(&document(1), None).unwrap();
    drop(owner);
    let found = only(&dir);
    let claimed = Recovery::claim(&found).unwrap();
    assert!(claimed.document.same_semantics(&document(1)));
    assert!(Recovery::scan(&dir).is_empty());
    assert!(Recovery::claim(&found).is_err());
    assert!(Recovery::discard(&dir, found.session).is_err());
    drop(claimed);
    assert!(
        found.checkpoint.exists(),
        "cancelling a claim keeps the original checkpoint"
    );
    let claimed = Recovery::claim(&found).unwrap();
    claimed.lease.discard().unwrap();
    assert!(!found.checkpoint.exists());
    fs::remove_dir_all(dir).unwrap();
}

#[test]
fn a_claim_rejects_changed_content_or_a_durable_discard_after_the_offer() {
    let dir = scratch();
    let owner = Recovery::start(&dir).unwrap();
    owner.checkpoint_document(&document(1), None).unwrap();
    drop(owner);
    let found = only(&dir);
    let original = fs::read(&found.checkpoint).unwrap();
    fs::write(&found.checkpoint, crate::pcnd::write(&document(2)).unwrap()).unwrap();
    assert_eq!(
        Recovery::claim(&found).unwrap_err().code,
        ErrorCode::ConflictStaleBase
    );
    fs::write(&found.checkpoint, original).unwrap();
    fs::write(tombstone(&dir, found.session), cleared_bytes(found.session)).unwrap();
    assert_eq!(
        Recovery::claim(&found).unwrap_err().code,
        ErrorCode::ConflictStaleBase
    );
    fs::remove_dir_all(dir).unwrap();
}

#[test]
fn a_claim_cannot_relabel_an_arbitrary_file_as_a_recovery_payload() {
    let dir = scratch();
    let owner = Recovery::start(&dir).unwrap();
    owner.checkpoint_document(&document(1), None).unwrap();
    drop(owner);
    let mut found = only(&dir);
    let unrelated = dir.join("unrelated.pcnd");
    fs::copy(&found.checkpoint, &unrelated).unwrap();
    found.checkpoint = unrelated.clone();
    assert_eq!(
        Recovery::claim(&found).unwrap_err().code,
        ErrorCode::TargetInvalid
    );
    assert!(unrelated.exists());
    fs::remove_dir_all(dir).unwrap();
}

#[test]
fn pcnd4_mask_checkpoint_scan_and_read_preserve_exact_source_and_coverage() {
    let bytes = include_bytes!("../../tests/pcnd-mask-fixtures/opaque-half.pcnd");
    let profile = paintcraft_core::types::resources::SharedBudget::new(0);
    let coverage = paintcraft_core::types::resources::SharedBudget::new(1 << 20);
    let doc = crate::pcnd::read_with_admission_checked(
        bytes,
        2,
        65536,
        1,
        crate::pcnd::ReadAdmission {
            metadata: crate::pcnd::MetadataAdmission {
                bytes: 0,
                budget: &profile,
            },
            coverage: crate::pcnd::CoverageAdmission {
                bytes: 1 << 20,
                budget: &coverage,
                limits: paintcraft_raster::coverage_mask::Limits::default(),
            },
        },
        &|| Ok(()),
    )
    .unwrap();
    let dir = scratch();
    let owner = Recovery::start(&dir).unwrap();
    owner.checkpoint_document(&doc, None).unwrap();
    drop(owner);
    let found = only(&dir);
    let restored = Recovery::read_checkpoint(&found.checkpoint).unwrap();
    assert!(doc.same_semantics(&restored));
    assert_eq!(crate::pcnd::write_compact(&restored).unwrap(), bytes);
    Recovery::discard(&dir, found.session).unwrap();
    fs::remove_dir_all(dir).unwrap();
}
