use super::*;
use std::{cell::Cell, fs};

#[test]
fn cancellation_after_encode_returns_ok_still_refuses_publication_and_keeps_destination_source_and_session()
 {
    let root =
        std::env::temp_dir().join(format!("paintcraft-export-cancel-{}", uuid::Uuid::new_v4()));
    fs::create_dir(&root).unwrap();
    let source = root.join("source.pcnd");
    let output = root.join("existing.jpg");
    let document = new_document(2, 1, None, 0, POLICY).unwrap();
    let original = pcnd::write(&document).unwrap();
    fs::write(&source, &original).unwrap();
    fs::write(&output, b"old output").unwrap();
    let (_, sha, identity) = read_captured(&source, INPUT_MAX, "project").unwrap();
    let mut session = Session::new(document);
    session.mark_unsaved();
    let snapshot = session.snapshot();
    let ticket = session.ticket();
    let binding = (source.clone(), sha);
    let state = State {
        session: Some(session),
        source: Some(binding.clone()),
        project_identity: Some(identity),
        ..Default::default()
    };
    let cancelled = Cell::new(false);
    let check = || {
        if cancelled.get() {
            Err(Error::new(
                ErrorCode::OperationCancelled,
                "test export cancellation",
            ))
        } else {
            Ok(())
        }
    };
    let outcome = state
        .write_copy_with_checked(
            &output,
            true,
            |sink| {
                sink.write_all(b"encoded new output").unwrap();
                // Deterministic cancellation after the encoder's final checkpoint.
                cancelled.set(true);
                Ok(())
            },
            &check,
        )
        .unwrap();
    assert_eq!(
        outcome.replacement,
        paintcraft_platform::save::Replacement::NotReplaced
    );
    assert!(
        outcome
            .reasons
            .iter()
            .any(|reason| reason.contains("test export cancellation"))
    );
    assert_eq!(fs::read(&output).unwrap(), b"old output");
    assert_eq!(fs::read(&source).unwrap(), original);
    assert_eq!(state.source.as_ref(), Some(&binding));
    let current = state.session.as_ref().unwrap();
    assert!(current.is_dirty());
    assert_eq!(current.ticket(), ticket);
    assert!(std::sync::Arc::ptr_eq(&snapshot, &current.snapshot()));
    assert!(
        state
            .project_identity
            .as_ref()
            .unwrap()
            .matches_path(&source)
            .unwrap()
    );
    assert_eq!(
        fs::read_dir(&root).unwrap().count(),
        2,
        "staged output must be cleaned up"
    );
    assert_eq!(
        state
            .write_copy_with_checked(
                &output,
                true,
                |_| panic!("pre-staging cancel must not encode"),
                &check
            )
            .unwrap_err()
            .code,
        ErrorCode::OperationCancelled
    );
    assert_eq!(fs::read_dir(&root).unwrap().count(), 2);
    let legacy = root.join("legacy-copy.jpg");
    assert!(
        state
            .write_copy_with(&legacy, false, |sink| {
                sink.write_all(b"legacy copy").unwrap();
                Ok(())
            })
            .unwrap()
            .is_success()
    );
    assert_eq!(fs::read(legacy).unwrap(), b"legacy copy");
    fs::remove_dir_all(root).unwrap();
}
