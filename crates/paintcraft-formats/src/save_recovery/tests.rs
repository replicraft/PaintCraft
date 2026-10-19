use super::*;
use crate::{pcnd, recovery::Recovery};

fn scratch() -> PathBuf {
    let path = std::env::temp_dir().join(format!("paintcraft-save-journal-{}", Uuid::new_v4()));
    fs::create_dir(&path).unwrap();
    fs::canonicalize(path).unwrap()
}
fn stamp() -> Stamp {
    Stamp {
        session: Uuid::new_v4(),
        sequence: 7,
        generation: 3,
        operation: "save-as",
    }
}
fn document() -> Document {
    paintcraft_document::new_document(4, 3, None, 0, paintcraft_document::POLICY).unwrap()
}

#[test]
fn orphaned_save_snapshot_is_discovered_and_claimed_without_its_original() {
    let root = scratch();
    let doc = document();
    let requested = root.join("original.pcnd");
    let saved = SavedAttempt::capture(&root, &doc, &requested, stamp()).unwrap();
    assert!(
        Recovery::scan(&root).is_empty(),
        "live owners are not offered"
    );
    let outcome = save::save_with_preparation(
        &requested,
        true,
        SNAPSHOT_MAX,
        |sink| pcnd::write_to(&doc, sink),
        |outcome| saved.prepare(outcome),
    );
    assert!(outcome.is_success());
    // Simulate a crash after replacement but before a finished record.
    let owner = saved.session();
    drop(saved);
    fs::remove_file(&requested).unwrap();
    let found = Recovery::scan(&root);
    assert_eq!(found.len(), 1);
    assert_eq!(found[0].session, owner);
    let evidence = read_record(&found[0]).unwrap();
    assert_eq!(evidence.get("phase"), Some(&Value::str("prepared")));
    let receipt = evidence.get("receipt").unwrap();
    assert_eq!(receipt.get("replacement"), Some(&Value::str("unknown")));
    assert_eq!(receipt.get("acknowledged_state_id"), Some(&Value::Null));
    assert_eq!(receipt.get("save_sequence"), Some(&Value::str("7")));
    assert_eq!(receipt.get("binding_generation"), Some(&Value::str("3")));
    let claimed = Recovery::claim(&found[0]).unwrap();
    assert_eq!(claimed.document.state_id, doc.state_id);
    claimed.lease.discard().unwrap();
    assert!(Recovery::scan(&root).is_empty());
    assert!(!record_path(&root.join(DIRECTORY), owner).exists());
    fs::remove_dir_all(root).unwrap();
}

#[test]
fn a_failed_prepare_never_replaces_the_destination() {
    let root = scratch();
    let requested = root.join("original.pcnd");
    fs::write(&requested, b"old").unwrap();
    let out = save::save_with_preparation(
        &requested,
        true,
        SNAPSHOT_MAX,
        |sink| pcnd::write_to(&document(), sink),
        |_| {
            Err(Error::new(
                ErrorCode::StorageFailed,
                "injected retention failure",
            ))
        },
    );
    assert_eq!(out.replacement, save::Replacement::NotReplaced);
    assert_eq!(fs::read(&requested).unwrap(), b"old");
    fs::remove_dir_all(root).unwrap();
}

#[test]
fn finished_receipt_keeps_identity_and_discard_removes_only_the_owned_store() {
    let root = scratch();
    let requested = root.join("original.pcnd");
    let saved = SavedAttempt::capture(&root, &document(), &requested, stamp()).unwrap();
    let out = save::save_with_preparation(
        &requested,
        true,
        SNAPSHOT_MAX,
        |sink| pcnd::write_to(&document(), sink),
        |outcome| saved.prepare(outcome),
    );
    saved.finish(&out).unwrap();
    let owner = saved.session();
    drop(saved);
    let found = Recovery::scan(&root).pop().unwrap();
    let body = read_record(&found).unwrap();
    assert_eq!(body.get("phase"), Some(&Value::str("finished")));
    assert_eq!(
        body.get("receipt").unwrap().get("operation_id"),
        out.receipt(&save::Context {
            operation: "save-as",
            source_state: found.state_id,
            source_sha256: None,
            losses_acknowledged: vec![],
            destination: &requested,
        })
        .get("operation_id")
    );
    fs::write(root.join(DIRECTORY).join("unrelated"), b"keep").unwrap();
    Recovery::discard(found.checkpoint.parent().unwrap(), owner).unwrap();
    assert_eq!(
        fs::read(root.join(DIRECTORY).join("unrelated")).unwrap(),
        b"keep"
    );
    assert!(requested.exists());
    fs::remove_dir_all(root).unwrap();
}

#[test]
fn corrupt_evidence_does_not_destroy_the_independently_validated_document_snapshot() {
    let root = scratch();
    let saved = SavedAttempt::capture(&root, &document(), &root.join("a.pcnd"), stamp()).unwrap();
    let owner = saved.session();
    drop(saved);
    let path = record_path(&root.join(DIRECTORY), owner);
    fs::write(&path, b"corrupt").unwrap();
    let found = Recovery::scan(&root).pop().unwrap();
    assert!(read_record(&found).is_err());
    let claimed = Recovery::claim(&found).unwrap();
    assert_eq!(claimed.document.canvas, (4, 3));
    assert!(
        path.exists(),
        "corrupt evidence is retained until explicit discard"
    );
    drop(claimed);
    fs::remove_dir_all(root).unwrap();
}

#[test]
fn admission_refuses_full_or_busy_stores_before_any_user_file_is_written() {
    let root = scratch();
    let directory = root.join(DIRECTORY);
    fs::create_dir(&directory).unwrap();
    let lock = OpenOptions::new()
        .read(true)
        .write(true)
        .create_new(true)
        .open(directory.join(".admission.lock"))
        .unwrap();
    lock.try_lock().unwrap();
    assert_eq!(
        SavedAttempt::capture(&root, &document(), &root.join("a.pcnd"), stamp())
            .unwrap_err()
            .code,
        ErrorCode::ResourceLimit
    );
    drop(lock);
    for _ in 0..OWNERS_MAX {
        fs::write(directory.join(format!("{}.lock", Uuid::new_v4())), b"").unwrap();
    }
    assert_eq!(
        SavedAttempt::capture(&root, &document(), &root.join("a.pcnd"), stamp())
            .unwrap_err()
            .code,
        ErrorCode::ResourceLimit
    );
    assert!(!root.join("a.pcnd").exists());
    fs::remove_dir_all(root).unwrap();
}

#[cfg(unix)]
#[test]
fn symlinked_retention_directories_are_rejected_and_not_scanned() {
    use std::os::unix::fs::symlink;
    let root = scratch();
    let elsewhere = scratch();
    symlink(&elsewhere, root.join(DIRECTORY)).unwrap();
    assert!(SavedAttempt::capture(&root, &document(), &root.join("a.pcnd"), stamp()).is_err());
    assert!(Recovery::scan(&root).is_empty());
    assert!(!Recovery::scan_report(&root).issues.is_empty());
    assert_eq!(fs::read_dir(&elsewhere).unwrap().count(), 0);
    fs::remove_dir_all(root).unwrap();
    fs::remove_dir_all(elsewhere).unwrap();
}

#[test]
fn diagnostic_record_validation_rejects_rechecksummed_wrong_stamps_and_future_fields() {
    let root = scratch();
    let saved = SavedAttempt::capture(&root, &document(), &root.join("a.pcnd"), stamp()).unwrap();
    let owner = saved.session();
    drop(saved);
    let found = Recovery::scan(&root).pop().unwrap();
    let path = record_path(&root.join(DIRECTORY), owner);
    let original = json::parse(&fs::read(&path).unwrap()).unwrap();
    for (key, wrong) in [
        ("save_sequence", Value::str("07")),
        ("operation", Value::str("export")),
        ("editor_session", Value::str("bad-id")),
        ("phase", Value::str("future-version")),
        ("document_id", Value::str(Uuid::new_v4().to_string())),
        ("unknown_field", Value::Null),
    ] {
        let Value::Object(mut envelope) = original.clone() else {
            unreachable!()
        };
        let Value::Object(mut fields) = envelope.remove("body").unwrap() else {
            unreachable!()
        };
        fields.insert(key.into(), wrong);
        let body = Value::Object(fields);
        envelope.insert("sha256".into(), Value::str(hash(&json::canonical(&body))));
        envelope.insert("body".into(), body);
        fs::write(&path, json::canonical(&Value::Object(envelope))).unwrap();
        assert!(read_record(&found).is_err(), "{key}");
        assert!(found.checkpoint.exists());
    }
    fs::remove_dir_all(root).unwrap();
}

#[test]
fn disk_reservation_rejects_a_full_store_before_creating_an_owner() {
    let root = scratch();
    let directory = root.join(DIRECTORY);
    fs::create_dir(&directory).unwrap();
    // A sparse file exercises metadata admission without consuming 4 GiB of disk.
    File::create(directory.join("retained-data"))
        .unwrap()
        .set_len(DISK_MAX)
        .unwrap();
    assert_eq!(
        SavedAttempt::capture(&root, &document(), &root.join("a.pcnd"), stamp())
            .unwrap_err()
            .code,
        ErrorCode::ResourceLimit
    );
    assert_eq!(fs::read_dir(&directory).unwrap().count(), 2);
    fs::remove_dir_all(root).unwrap();
}

#[test]
fn a_new_nested_recovery_namespace_is_created_before_publishing_a_snapshot() {
    let scratch = scratch();
    let root = scratch.join("new/parents/recovery");
    let saved =
        SavedAttempt::capture(&root, &document(), &scratch.join("a.pcnd"), stamp()).unwrap();
    drop(saved);
    let found = Recovery::scan(&root);
    assert_eq!(found.len(), 1);
    Recovery::claim(&found[0]).unwrap().lease.discard().unwrap();
    fs::remove_dir_all(scratch).unwrap();
}
