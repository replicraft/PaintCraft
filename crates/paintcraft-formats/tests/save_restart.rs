//! Real process exit releases the save snapshot claim without running destructors.
//! This checks restart retention; it is not power-loss/filesystem qualification.
use paintcraft_core::types::json::Value;
use paintcraft_formats::{
    pcnd,
    recovery::Recovery,
    save,
    save_recovery::{SavedAttempt, Stamp, read_record},
};
use std::{fs, path::PathBuf, process::Command};
use uuid::Uuid;

#[test]
fn process_exit_after_replacement_preserves_unknown_receipt_and_source_independent_snapshot() {
    let root = std::env::temp_dir().join(format!("paintcraft-save-exit-{}", Uuid::new_v4()));
    fs::create_dir(&root).unwrap();
    let status = Command::new(std::env::current_exe().unwrap())
        .args(["--exact", "abandoned_save_helper", "--ignored"])
        .env("PAINTCRAFT_TEST_SAVE_EXIT_ROOT", &root)
        .output()
        .unwrap();
    assert_eq!(
        status.status.code(),
        Some(23),
        "{}",
        String::from_utf8_lossy(&status.stderr)
    );
    fs::remove_file(root.join("original.pcnd")).unwrap();
    let found = Recovery::scan(&root);
    assert_eq!(found.len(), 1);
    let evidence = read_record(&found[0]).unwrap();
    assert_eq!(evidence.get("phase"), Some(&Value::str("prepared")));
    assert_eq!(
        evidence.get("receipt").unwrap().get("replacement"),
        Some(&Value::str("unknown"))
    );
    assert_eq!(
        evidence
            .get("receipt")
            .unwrap()
            .get("acknowledged_state_id"),
        Some(&Value::Null)
    );
    let claim = Recovery::claim(&found[0]).unwrap();
    assert_eq!(claim.document.canvas, (4, 3));
    claim.lease.discard().unwrap();
    fs::remove_dir_all(root).unwrap();
}

#[test]
#[ignore = "subprocess-only helper; the parent test verifies its exit and durable artifacts"]
fn abandoned_save_helper() {
    let root = PathBuf::from(
        std::env::var_os("PAINTCRAFT_TEST_SAVE_EXIT_ROOT")
            .expect("parent test supplies isolated storage"),
    );
    let document =
        paintcraft_document::new_document(4, 3, None, 0, paintcraft_document::POLICY).unwrap();
    let destination = root.join("original.pcnd");
    let retained = SavedAttempt::capture(
        &root,
        &document,
        &destination,
        Stamp {
            session: Uuid::new_v4(),
            sequence: 11,
            generation: 2,
            operation: "save-as",
        },
    )
    .unwrap();
    let outcome = save::save_with_preparation(
        &destination,
        true,
        1 << 30,
        |sink| pcnd::write_to(&document, sink),
        |outcome| retained.prepare(outcome),
    );
    assert!(outcome.is_success(), "{:?}", outcome.reasons);
    std::process::exit(23); // Skip finish and Drop; preserve the prepared uncertainty.
}
