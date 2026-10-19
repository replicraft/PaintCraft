use super::*;
use crate::{
    pcnd,
    save_recovery::tests::{document, scratch, stamp},
};

fn orphan(root: &Path, destination: &Path, unknown: bool) -> (Found, Value) {
    let doc = document();
    let retained = SavedAttempt::capture(root, &doc, destination, stamp()).unwrap();
    let mut outcome = save::save_with_preparation(
        destination,
        true,
        SNAPSHOT_MAX,
        |sink| pcnd::write_to(&doc, sink),
        |outcome| retained.prepare(outcome),
    );
    assert!(outcome.is_success());
    // To model loss of the final receipt, leave only the real prepared record.
    if !unknown {
        outcome.durable = false;
        retained.finish(&outcome).unwrap();
    }
    drop(retained);
    let found = Recovery::scan(root).pop().unwrap();
    let receipt = read_record(&found).unwrap().get("receipt").unwrap().clone();
    (found, receipt)
}

#[test]
fn resumed_confirmation_updates_the_original_receipt_and_retains_recovery() {
    let root = scratch();
    let destination = root.join("a.pcnd");
    let (found, before) = orphan(&root, &destination, false);
    let checked = reconcile(&found, &destination, save::ReconcileMode::ConfirmDurability).unwrap();
    assert_eq!(checked.observation, save::Observation::Staged);
    for key in [
        "operation_id",
        "save_sequence",
        "binding_generation",
        "source_state_id",
    ] {
        assert_eq!(before.get(key), checked.receipt.get(key), "{key}");
    }
    assert_eq!(
        checked.receipt.get("durability"),
        Some(&Value::str("confirmed"))
    );
    assert_eq!(
        read_record(&found).unwrap().get("receipt"),
        Some(&checked.receipt)
    );
    assert!(
        found.checkpoint.exists(),
        "inspection never retires the source snapshot"
    );
    assert_eq!(
        Recovery::claim(&found).unwrap().document.state_id,
        found.state_id
    );
    fs::remove_dir_all(root).unwrap();
}

#[test]
fn an_abandoned_prepared_attempt_stays_unknown_even_when_its_bytes_are_present() {
    let root = scratch();
    let destination = root.join("a.pcnd");
    let (found, _) = orphan(&root, &destination, true);
    let checked = reconcile(&found, &destination, save::ReconcileMode::ConfirmDurability).unwrap();
    assert_eq!(checked.observation, save::Observation::Staged);
    assert_eq!(
        checked.receipt.get("replacement"),
        Some(&Value::str("unknown"))
    );
    assert_eq!(
        checked.receipt.get("acknowledged_state_id"),
        Some(&Value::Null)
    );
    assert!(found.checkpoint.exists());
    fs::remove_dir_all(root).unwrap();
}

#[test]
fn selection_of_another_file_cannot_inspect_or_modify_it() {
    let root = scratch();
    let destination = root.join("a.pcnd");
    let (found, original) = orphan(&root, &destination, false);
    let other = root.join("other.pcnd");
    fs::write(&other, b"untouched").unwrap();
    assert_eq!(
        reconcile(&found, &other, save::ReconcileMode::ConfirmDurability)
            .unwrap_err()
            .code,
        ErrorCode::ConflictStaleBase
    );
    assert_eq!(fs::read(&other).unwrap(), b"untouched");
    assert_eq!(read_record(&found).unwrap().get("receipt"), Some(&original));
    fs::remove_dir_all(root).unwrap();
}

#[test]
fn source_claims_and_captured_identity_are_revalidated_for_every_reconciliation() {
    let root = scratch();
    let destination = root.join("a.pcnd");
    let (found, _) = orphan(&root, &destination, false);
    let held = Recovery::claim(&found).unwrap();
    assert!(reconcile(&found, &destination, save::ReconcileMode::Inspect).is_err());
    drop(held);
    fs::write(&found.checkpoint, b"changed source").unwrap();
    assert!(reconcile(&found, &destination, save::ReconcileMode::Inspect).is_err());
    assert!(found.checkpoint.exists());
    fs::remove_dir_all(root).unwrap();
}

#[test]
fn legacy_v1_unknown_receipts_upgrade_without_inventing_replacement_evidence() {
    let root = scratch();
    let destination = root.join("a.pcnd");
    let (found, original) = orphan(&root, &destination, true);
    let path = record_path(found.checkpoint.parent().unwrap(), found.session);
    let mut body = read_record(&found).unwrap();
    let Value::Object(fields) = &mut body else {
        unreachable!()
    };
    fields.remove("evidence");
    let bytes = json::canonical(&Value::object([
        ("schema", Value::str("paintcraft.saved-attempt/v1")),
        ("sha256", Value::str(hash(&json::canonical(&body)))),
        ("body", body),
    ]));
    fs::write(&path, bytes).unwrap();
    let checked = reconcile(&found, &destination, save::ReconcileMode::ConfirmDurability).unwrap();
    assert_eq!(
        checked.receipt.get("operation_id"),
        original.get("operation_id")
    );
    assert_eq!(
        checked.receipt.get("replacement"),
        Some(&Value::str("unknown"))
    );
    assert_eq!(
        checked.receipt.get("acknowledged_state_id"),
        Some(&Value::Null)
    );
    let upgraded = json::parse(&fs::read(&path).unwrap()).unwrap();
    assert_eq!(
        upgraded.get("schema"),
        Some(&Value::str("paintcraft.saved-attempt/v2"))
    );
    assert_eq!(
        read_record(&found)
            .unwrap()
            .get("evidence")
            .unwrap()
            .get("replacement_acknowledged"),
        Some(&Value::Bool(false))
    );
    fs::remove_dir_all(root).unwrap();
}

#[test]
fn a_changed_selected_alias_is_rejected_before_confirmation_and_metadata_update() {
    use std::os::unix::fs::symlink;
    let root = scratch();
    let destination = root.join("a.pcnd");
    let (found, _) = orphan(&root, &destination, false);
    let alias = root.join("alias.pcnd");
    symlink(&destination, &alias).unwrap();
    reconcile(&found, &alias, save::ReconcileMode::Inspect).unwrap();
    let other = root.join("other.pcnd");
    fs::write(&other, b"external").unwrap();
    fs::remove_file(&alias).unwrap();
    symlink(&other, &alias).unwrap();
    let before = read_record(&found).unwrap();
    assert_eq!(
        reconcile(&found, &alias, save::ReconcileMode::ConfirmDurability)
            .unwrap_err()
            .code,
        ErrorCode::ConflictStaleBase
    );
    assert_eq!(read_record(&found).unwrap(), before);
    assert_eq!(fs::read(&other).unwrap(), b"external");
    fs::remove_dir_all(root).unwrap();
}

#[test]
fn missing_or_third_party_content_is_observed_without_retrying_the_user_file_write() {
    let root = scratch();
    let destination = root.join("a.pcnd");
    let (found, _) = orphan(&root, &destination, false);
    fs::write(&destination, b"external").unwrap();
    let checked = reconcile(&found, &destination, save::ReconcileMode::ConfirmDurability).unwrap();
    assert_eq!(checked.observation, save::Observation::Other);
    assert_eq!(fs::read(&destination).unwrap(), b"external");
    fs::remove_file(&destination).unwrap();
    let checked = reconcile(&found, &destination, save::ReconcileMode::Inspect).unwrap();
    assert_eq!(checked.observation, save::Observation::Missing);
    assert!(!destination.exists());
    assert!(found.checkpoint.exists());
    fs::remove_dir_all(root).unwrap();
}

#[test]
fn source_digest_metadata_survives_reconciliation_without_rebinding_source_lineage() {
    let root = scratch();
    let destination = root.join("a.pcnd");
    let (found, _) = orphan(&root, &destination, false);
    let mut body = read_record(&found).unwrap();
    let Value::Object(fields) = &mut body else {
        unreachable!()
    };
    let Value::Object(receipt) = fields.get_mut("receipt").unwrap() else {
        unreachable!()
    };
    let source_digest = hash(b"original source bytes");
    receipt.insert("source_sha256".into(), Value::str(&source_digest));
    write_record(found.checkpoint.parent().unwrap(), found.session, &body).unwrap();
    let checked = reconcile(&found, &destination, save::ReconcileMode::Inspect).unwrap();
    assert_eq!(
        checked.receipt.get("source_sha256"),
        Some(&Value::str(source_digest))
    );
    assert_eq!(
        checked.receipt.get("source_lineage_adopted"),
        Some(&Value::Bool(false))
    );
    assert_eq!(
        checked.receipt.get("acknowledged_state_id"),
        Some(&Value::Null)
    );
    fs::remove_dir_all(root).unwrap();
}
