use super::*;
use crate::save::{Phase as SavePhase, tests::scratch};

fn incomplete(dest: &Path) -> Outcome {
    fs::write(dest, b"old").unwrap();
    save_impl(
        dest,
        true,
        1024,
        |sink| sink.write_all(b"new").map_err(io_error),
        |phase| {
            if phase == SavePhase::DirectorySync {
                Err(std::io::ErrorKind::StorageFull.into())
            } else {
                Ok(())
            }
        },
    )
}

fn receipt(outcome: &Outcome, path: &Path) -> Value {
    outcome.receipt_stamped(
        &Context {
            operation: "save-as",
            source_state: Uuid::nil(),
            source_sha256: None,
            losses_acknowledged: vec![],
            destination: path,
        },
        23,
        7,
    )
}

#[test]
fn inspect_keeps_the_original_attempt_and_cannot_invent_a_durability_barrier() {
    let dir = scratch("reconcile-inspect");
    let dest = dir.join("a.pcnd");
    let original = incomplete(&dest);
    let before = receipt(&original, &dest);
    let checked = original.reconcile(ReconcileMode::Inspect).unwrap();
    assert_eq!(checked.observation, Observation::Staged);
    assert_eq!(checked.outcome.replacement, Replacement::Replaced);
    assert!(!checked.outcome.durable);
    assert!(!original.durable);
    assert_eq!(
        receipt(&original, &dest),
        before,
        "original evidence is immutable"
    );
    let revised = receipt(&checked.outcome, &dest);
    for key in [
        "operation_id",
        "destination_id",
        "source_state_id",
        "save_sequence",
        "binding_generation",
    ] {
        assert_eq!(before.get(key), revised.get(key), "{key}");
    }
    assert_eq!(revised.get("acknowledged_state_id"), Some(&Value::Null));
    assert_eq!(fs::read(&dest).unwrap(), b"new");
    assert_eq!(fs::read_dir(&dir).unwrap().count(), 1);
    fs::remove_dir_all(dir).unwrap();
}

#[test]
fn acknowledged_replacement_can_finish_barriers_without_reencoding_or_replacement() {
    use std::os::unix::fs::MetadataExt;
    let dir = scratch("reconcile-durable");
    let dest = dir.join("a.pcnd");
    let original = incomplete(&dest);
    let identity = fs::metadata(&dest).unwrap().ino();
    let checked = original
        .reconcile(ReconcileMode::ConfirmDurability)
        .unwrap();
    assert_eq!(checked.observation, Observation::Staged);
    assert!(checked.outcome.is_success());
    assert_eq!(
        fs::metadata(&dest).unwrap().ino(),
        identity,
        "no new replacement"
    );
    assert_eq!(fs::read_dir(&dir).unwrap().count(), 1);
    let revised = receipt(&checked.outcome, &dest);
    assert_eq!(
        revised.get("acknowledged_state_id"),
        Some(&Value::str(Uuid::nil().to_string()))
    );
    assert_eq!(
        revised.get("operation_id"),
        receipt(&original, &dest).get("operation_id")
    );
    assert!(checked.outcome.reconcile(ReconcileMode::Inspect).is_err());
    fs::remove_dir_all(dir).unwrap();
}

#[test]
fn hashes_alone_cannot_prove_an_unacknowledged_replace_even_for_identical_bytes() {
    let dir = scratch("reconcile-no-ack");
    let dest = dir.join("a.pcnd");
    let mut original = incomplete(&dest);
    original.replacement = Replacement::Unknown;
    let attempt = original.attempt.as_mut().unwrap();
    attempt.replacement_acknowledged = false;
    // The same content may have been present before the request was ever made.
    attempt.original = Some(attempt.staged.clone());
    let checked = reconcile(&original, ReconcileMode::ConfirmDurability, |phase| {
        assert_eq!(
            phase,
            Phase::Observe,
            "no persistence writes for an unacknowledged attempt"
        );
        Ok(())
    })
    .unwrap();
    assert_eq!(checked.observation, Observation::Staged);
    assert_eq!(checked.outcome.replacement, Replacement::Unknown);
    assert!(!checked.outcome.is_success());
    let r = receipt(&checked.outcome, &dest);
    assert_eq!(r.get("acknowledged_state_id"), Some(&Value::Null));
    assert_eq!(r.get("retry"), Some(&Value::str("inspect")));
    fs::remove_dir_all(dir).unwrap();
}

#[test]
fn reinspection_distinguishes_original_third_missing_and_unreadable_states() {
    let dir = scratch("reconcile-observations");
    let dest = dir.join("a.pcnd");
    let original = incomplete(&dest);
    for (content, expected) in [
        (b"old".as_slice(), Observation::Original),
        (b"third", Observation::Other),
    ] {
        fs::write(&dest, content).unwrap();
        let checked = original
            .reconcile(ReconcileMode::ConfirmDurability)
            .unwrap();
        assert_eq!(checked.observation, expected);
        assert_eq!(checked.outcome.replacement, Replacement::Unknown);
        assert!(!checked.outcome.is_success());
        assert_eq!(fs::read(&dest).unwrap(), content, "never retry the write");
    }
    fs::remove_file(&dest).unwrap();
    let checked = original.reconcile(ReconcileMode::Inspect).unwrap();
    assert_eq!(checked.observation, Observation::Missing);
    assert!(!checked.outcome.durable);
    fs::create_dir(&dest).unwrap();
    let checked = original.reconcile(ReconcileMode::Inspect).unwrap();
    assert_eq!(checked.observation, Observation::Unreadable);
    assert_eq!(checked.outcome.observed_sha256, None);
    fs::remove_dir_all(dir).unwrap();
}

#[test]
fn barrier_failure_preserves_incomplete_status_and_never_retries_replacement() {
    let dir = scratch("reconcile-barriers");
    let dest = dir.join("a.pcnd");
    let original = incomplete(&dest);
    for failed in [
        Phase::Observe,
        Phase::FileSync,
        Phase::DirectorySync,
        Phase::Validate,
    ] {
        let checked = reconcile(&original, ReconcileMode::ConfirmDurability, |phase| {
            if phase == failed {
                Err(std::io::ErrorKind::PermissionDenied.into())
            } else {
                Ok(())
            }
        })
        .unwrap();
        assert!(!checked.outcome.is_success());
        assert_eq!(
            receipt(&checked.outcome, &dest).get("acknowledged_state_id"),
            Some(&Value::Null)
        );
        if failed == Phase::Validate {
            assert_eq!(checked.observation, Observation::Unreadable);
            assert_eq!(checked.outcome.observed_sha256, None);
        }
        assert_eq!(fs::read(&dest).unwrap(), b"new");
        assert_eq!(fs::read_dir(&dir).unwrap().count(), 1);
    }
    fs::remove_dir_all(dir).unwrap();
}

#[test]
fn syncing_a_descriptor_is_not_confirmation_of_a_different_file_at_the_same_path() {
    let dir = scratch("reconcile-replaced-descriptor");
    let dest = dir.join("a.pcnd");
    let original = incomplete(&dest);
    let checked = reconcile(&original, ReconcileMode::ConfirmDurability, |phase| {
        if phase == Phase::Validate {
            let third = dir.join("third");
            // Even identical bytes do not make syncing the old descriptor a
            // durability barrier for a different inode published afterward.
            fs::write(&third, b"new")?;
            fs::rename(third, &dest)?;
        }
        Ok(())
    })
    .unwrap();
    assert_eq!(checked.observation, Observation::Unreadable);
    assert_eq!(checked.outcome.replacement, Replacement::Unknown);
    assert!(!checked.outcome.durable);
    fs::remove_dir_all(dir).unwrap();
}

#[test]
fn edits_and_deletion_during_final_validation_do_not_confirm_an_earlier_hash() {
    let dir = scratch("reconcile-validation");
    for delete in [false, true] {
        let dest = dir.join(if delete {
            "removed.pcnd"
        } else {
            "edited.pcnd"
        });
        let original = incomplete(&dest);
        let checked = reconcile(&original, ReconcileMode::ConfirmDurability, |phase| {
            if phase == Phase::Validate {
                if delete {
                    fs::remove_file(&dest)?;
                } else {
                    fs::write(&dest, b"third")?;
                }
            }
            Ok(())
        })
        .unwrap();
        assert_eq!(checked.observation, Observation::Unreadable);
        assert_eq!(checked.outcome.replacement, Replacement::Unknown);
        assert!(!checked.outcome.durable);
    }
    fs::remove_dir_all(dir).unwrap();
}

#[test]
fn captured_destination_is_not_retargeted_by_an_alias_or_public_diagnostic_hash() {
    use std::os::unix::fs::symlink;
    let dir = scratch("reconcile-alias");
    let dest = dir.join("actual.pcnd");
    let alias = dir.join("alias.pcnd");
    symlink(&dest, &alias).unwrap();
    fs::write(&dest, b"old").unwrap();
    let mut original = save_impl(
        &alias,
        true,
        1024,
        |sink| sink.write_all(b"new").map_err(io_error),
        |phase| {
            if phase == SavePhase::ReadBack {
                Err(std::io::ErrorKind::StorageFull.into())
            } else {
                Ok(())
            }
        },
    );
    let other = dir.join("other.pcnd");
    fs::write(&other, b"other").unwrap();
    fs::remove_file(&alias).unwrap();
    symlink(&other, &alias).unwrap();
    original.staged_sha256 = Some("forged".into());
    let checked = original
        .reconcile(ReconcileMode::ConfirmDurability)
        .unwrap();
    assert!(checked.outcome.is_success());
    assert_eq!(
        checked.outcome.staged_sha256,
        checked.outcome.observed_sha256
    );
    assert_eq!(
        receipt(&checked.outcome, &alias).get("destination_id"),
        receipt(&original, &dest).get("destination_id")
    );
    assert_eq!(fs::read(&other).unwrap(), b"other");
    fs::remove_dir_all(dir).unwrap();
}

#[test]
fn replacement_path_becoming_a_symlink_is_not_accepted_as_a_captured_namespace() {
    use std::os::unix::fs::symlink;
    let dir = scratch("reconcile-new-symlink");
    let dest = dir.join("a.pcnd");
    let original = incomplete(&dest);
    let other = dir.join("other.pcnd");
    fs::write(&other, b"new").unwrap();
    fs::remove_file(&dest).unwrap();
    symlink(&other, &dest).unwrap();
    let checked = original
        .reconcile(ReconcileMode::ConfirmDurability)
        .unwrap();
    assert_eq!(checked.observation, Observation::Unreadable);
    assert!(!checked.outcome.is_success());
    fs::remove_dir_all(dir).unwrap();
}

#[test]
fn repeated_inspection_diagnostics_are_bounded_and_nonattempts_cannot_reconcile() {
    let dir = scratch("reconcile-bounds");
    let dest = dir.join("a.pcnd");
    let mut outcome = incomplete(&dest);
    let base = outcome.reasons.len();
    for _ in 0..40 {
        outcome = outcome.reconcile(ReconcileMode::Inspect).unwrap().outcome;
        assert!(outcome.reasons.len() <= base + 2);
    }
    assert!(
        save(&dest, b"other", false)
            .reconcile(ReconcileMode::Inspect)
            .is_err()
    );
    assert!(
        not_attempted(&Error::new(ErrorCode::CodecFailed, "no bytes"))
            .reconcile(ReconcileMode::Inspect)
            .is_err()
    );
    fs::remove_dir_all(dir).unwrap();
}

#[test]
fn a_link_count_change_after_sync_is_not_a_new_persistence_barrier() {
    let dir = scratch("reconcile-identical-late-edit");
    let dest = dir.join("a.pcnd");
    let original = incomplete(&dest);
    let checked = reconcile(&original, ReconcileMode::ConfirmDurability, |phase| {
        if phase == Phase::Validate {
            // Link count changes even when the filesystem reports the same
            // timestamps for edits within one clock tick. Content and inode stay.
            fs::hard_link(&dest, dir.join("extra-link"))?;
            fs::write(&dest, b"new")?;
        }
        Ok(())
    })
    .unwrap();
    assert_eq!(checked.observation, Observation::Unreadable);
    assert_eq!(checked.outcome.replacement, Replacement::Unknown);
    assert!(!checked.outcome.is_success());
    fs::remove_dir_all(dir).unwrap();
}
