use super::*;
use crate::save::{Phase, tests::scratch};

fn incomplete(path: &Path) -> Outcome {
    fs::write(path, b"old").unwrap();
    save_impl(
        path,
        true,
        1024,
        |sink| sink.write_all(b"new").map_err(io_error),
        |phase| {
            if phase == Phase::DirectorySync {
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
fn reacquire(outcome: &Outcome, path: &Path) -> Outcome {
    Outcome::reacquire(
        &receipt(outcome, path),
        Some(&outcome.reconciliation_evidence()),
        path,
        outcome.staged_sha256.as_deref().unwrap(),
        3,
    )
    .unwrap()
}

#[test]
fn persisted_evidence_reacquires_the_same_attempt_without_repeating_replacement() {
    use std::os::unix::fs::MetadataExt;
    let dir = scratch("resume-save");
    let path = dir.join("a.pcnd");
    let original = incomplete(&path);
    let before = fs::metadata(&path).unwrap().ino();
    let resumed = reacquire(&original, &path);
    let checked = resumed.reconcile(ReconcileMode::ConfirmDurability).unwrap();
    assert!(checked.outcome.is_success());
    assert_eq!(fs::metadata(&path).unwrap().ino(), before);
    for key in [
        "operation_id",
        "destination_id",
        "save_sequence",
        "binding_generation",
        "source_state_id",
    ] {
        assert_eq!(
            receipt(&original, &path).get(key),
            receipt(&checked.outcome, &path).get(key),
            "{key}"
        );
    }
    fs::remove_dir_all(dir).unwrap();
}

#[test]
fn the_reacquired_namespace_must_match_and_no_stored_field_supplies_a_path() {
    let dir = scratch("resume-other");
    let path = dir.join("a.pcnd");
    let other = dir.join("other.pcnd");
    let original = incomplete(&path);
    fs::write(&other, b"new").unwrap();
    assert_eq!(
        Outcome::reacquire(
            &receipt(&original, &path),
            Some(&original.reconciliation_evidence()),
            &other,
            original.staged_sha256.as_deref().unwrap(),
            3
        )
        .unwrap_err()
        .code,
        ErrorCode::ConflictStaleBase
    );
    let Value::Object(fields) = original.reconciliation_evidence() else {
        unreachable!()
    };
    assert!(!fields.contains_key("path"));
    assert_eq!(fs::read(&other).unwrap(), b"new");
    fs::remove_dir_all(dir).unwrap();
}

#[test]
fn original_content_observation_survives_serialization() {
    let dir = scratch("resume-original");
    let path = dir.join("a.pcnd");
    let original = incomplete(&path);
    fs::write(&path, b"old").unwrap();
    let resumed = reacquire(&original, &path);
    let checked = resumed.reconcile(ReconcileMode::Inspect).unwrap();
    assert_eq!(checked.observation, Observation::Original);
    assert!(!checked.outcome.is_success());
    assert_eq!(fs::read(&path).unwrap(), b"old");
    fs::remove_dir_all(dir).unwrap();
}

#[test]
fn a_prepared_unknown_receipt_never_gains_acknowledgement_from_matching_content() {
    let dir = scratch("resume-unknown");
    let path = dir.join("a.pcnd");
    let mut original = incomplete(&path);
    original.replacement = Replacement::Unknown;
    original.attempt.as_mut().unwrap().replacement_acknowledged = false;
    let checked = reacquire(&original, &path)
        .reconcile(ReconcileMode::ConfirmDurability)
        .unwrap();
    assert_eq!(checked.observation, Observation::Staged);
    assert!(!checked.outcome.is_success());
    assert_eq!(
        receipt(&checked.outcome, &path).get("acknowledged_state_id"),
        Some(&Value::Null)
    );
    fs::remove_dir_all(dir).unwrap();
}

#[test]
fn malformed_evidence_or_a_mismatched_snapshot_is_rejected_before_inspection() {
    let dir = scratch("resume-validation");
    let path = dir.join("a.pcnd");
    let original = incomplete(&path);
    for (key, wrong) in [
        ("schema", Value::str("future")),
        ("operation_id", Value::str(Uuid::new_v4().to_string())),
        ("original_reason_count", Value::str("999")),
        ("replacement_acknowledged", Value::Bool(false)),
    ] {
        let Value::Object(mut fields) = original.reconciliation_evidence() else {
            unreachable!()
        };
        fields.insert(key.into(), wrong);
        assert!(
            Outcome::reacquire(
                &receipt(&original, &path),
                Some(&Value::Object(fields)),
                &path,
                original.staged_sha256.as_deref().unwrap(),
                3
            )
            .is_err(),
            "{key}"
        );
    }
    assert!(
        Outcome::reacquire(
            &receipt(&original, &path),
            Some(&original.reconciliation_evidence()),
            &path,
            original.staged_sha256.as_deref().unwrap(),
            4
        )
        .is_err()
    );
    assert_eq!(fs::read(&path).unwrap(), b"new");
    fs::remove_dir_all(dir).unwrap();
}

#[test]
fn repeated_reacquisition_keeps_diagnostic_growth_bounded() {
    let dir = scratch("resume-bounded");
    let path = dir.join("a.pcnd");
    let mut original = incomplete(&path);
    let base = original.reasons.len();
    for _ in 0..80 {
        original = reacquire(&original, &path)
            .reconcile(ReconcileMode::Inspect)
            .unwrap()
            .outcome;
        assert!(original.reasons.len() <= base + 2);
    }
    fs::remove_dir_all(dir).unwrap();
}
