use super::*;
use std::fs;
#[test]
fn late_captured_source_alias_is_refused_at_publication_barrier() {
    let folder =
        std::env::temp_dir().join(format!("paintcraft-copy-barrier-{}", uuid::Uuid::new_v4()));
    fs::create_dir(&folder).unwrap();
    let source = folder.join("source.pcnd");
    let moved = folder.join("moved.pcnd");
    let output = folder.join("copy.png");
    fs::write(&source, b"captured project bytes").unwrap();
    fs::write(&output, b"old output").unwrap();
    let (_, sha, identity) = read_captured(&source, INPUT_MAX, "test source").unwrap();
    let state = State {
        source: Some((source.clone(), sha)),
        project_identity: Some(identity),
        ..State::default()
    };
    let outcome = state
        .write_copy_with(&output, true, |sink| {
            // Deterministic replacement after initial admission, during staged IO.
            fs::rename(&source, &moved).unwrap();
            fs::remove_file(&output).unwrap();
            fs::hard_link(&moved, &output).unwrap();
            sink.write_all(b"encoded new image").unwrap();
            Ok(())
        })
        .unwrap();
    assert!(!outcome.is_success());
    assert!(
        outcome
            .reasons
            .iter()
            .any(|reason| reason.contains("captured source project")),
        "{:?}",
        outcome.reasons
    );
    assert_eq!(fs::read(&output).unwrap(), b"captured project bytes");
    assert_eq!(fs::read(&moved).unwrap(), b"captured project bytes");
    assert_eq!(fs::read_dir(&folder).unwrap().count(), 2);
    let unrelated = folder.join("unrelated.png");
    assert!(
        state
            .write_copy_with(&unrelated, false, |sink| {
                sink.write_all(b"unrelated output").unwrap();
                Ok(())
            })
            .unwrap()
            .is_success()
    );
    assert_eq!(fs::read(unrelated).unwrap(), b"unrelated output");
    fs::remove_dir_all(folder).unwrap();
}

#[test]
fn actual_save_rechecks_import_identity_after_staging_without_advancing_session_or_binding() {
    let root = std::env::temp_dir().join(format!(
        "paintcraft-save-checkpoint-{}",
        uuid::Uuid::new_v4()
    ));
    fs::create_dir(&root).unwrap();
    let source = root.join("source.dds");
    let moved = root.join("moved.dds");
    let output = root.join("output.pcnd");
    let project = root.join("project.pcnd");
    fs::write(&source, b"retained imported bytes").unwrap();
    fs::write(&output, b"retained imported bytes").unwrap();
    let doc = new_document(2, 1, None, 0, POLICY).unwrap();
    fs::write(&project, pcnd::write(&doc).unwrap()).unwrap();
    let (_, sha, project_identity) = read_captured(&project, INPUT_MAX, "project").unwrap();
    let (_, _, identity) = read_captured(&source, INPUT_MAX, "import").unwrap();
    let mut session = Session::new(doc);
    session.mark_unsaved();
    let ticket = session.ticket();
    let snapshot = session.snapshot();
    let binding = (project.clone(), sha.clone());
    let mut state = State {
        session: Some(session),
        source: Some(binding.clone()),
        project_identity: Some(project_identity),
        import_source: Some(source.clone()),
        import_identity: Some(identity),
        ..State::default()
    };
    let (input, renamed, dest) = (source.clone(), moved.clone(), output.clone());
    crate::commands::native_save::inject_at_checkpoint(
        move || {
            fs::rename(input, &renamed).unwrap();
            fs::remove_file(&dest).unwrap();
            fs::hard_link(renamed, dest).unwrap();
        },
        || {
            let reply = state.save(output.clone(), true).unwrap();
            assert_eq!(reply.is_error, Some(true));
            let text = match &reply.content[0] {
                rmcp::model::ContentBlock::Text(text) => &text.text,
                _ => panic!("file receipt text"),
            };
            let value: serde_json::Value = serde_json::from_str(text).unwrap();
            assert_eq!(value["replacement"], "not-replaced");
            assert!(value["staged_sha256"].as_str().is_some());
            assert!(value["acknowledged_state_id"].is_null());
            assert_eq!(value["durability"], "not-confirmed");
            assert!(
                value["reasons"]
                    .to_string()
                    .contains("captured imported image")
            );
        },
    );
    let session = state.session.as_ref().unwrap();
    assert!(session.is_dirty());
    assert_eq!(session.ticket(), ticket);
    assert!(std::sync::Arc::ptr_eq(&snapshot, &session.snapshot()));
    assert_eq!(state.source.as_ref(), Some(&binding));
    assert!(
        state
            .project_identity
            .as_ref()
            .unwrap()
            .matches_path(&project)
            .unwrap()
    );
    assert_eq!(fs::read(&output).unwrap(), b"retained imported bytes");
    assert_eq!(fs::read(&moved).unwrap(), b"retained imported bytes");
    fs::remove_file(&output).unwrap();
    // Imported source identity may survive renaming; an unrelated Save As is valid.
    assert_ne!(
        state.save(output.clone(), false).unwrap().is_error,
        Some(true)
    );
    assert!(!state.session.as_ref().unwrap().is_dirty());
    assert_eq!(state.source.as_ref().unwrap().0, output);
    assert_eq!(fs::read_dir(&root).unwrap().count(), 3);
    fs::remove_dir_all(root).unwrap();
}

#[test]
fn inplace_save_combines_expected_project_digest_with_final_import_identity_guard() {
    let root = std::env::temp_dir().join(format!(
        "paintcraft-inplace-checkpoint-{}",
        uuid::Uuid::new_v4()
    ));
    fs::create_dir(&root).unwrap();
    let project = root.join("project.pcnd");
    let imported = root.join("imported-source");
    let moved = root.join("moved-source");
    let doc = new_document(2, 1, None, 0, POLICY).unwrap();
    let bytes = pcnd::write(&doc).unwrap();
    fs::write(&project, &bytes).unwrap();
    // An identical-content imported source defeats content-only final fingerprints.
    fs::write(&imported, &bytes).unwrap();
    let (_, sha, project_identity) = read_captured(&project, INPUT_MAX, "project").unwrap();
    let (_, _, import_identity) = read_captured(&imported, INPUT_MAX, "import").unwrap();
    let mut session = Session::new(doc);
    session.mark_unsaved();
    let ticket = session.ticket();
    let history = session.history_ticket();
    let binding = (project.clone(), sha);
    let mut state = State {
        session: Some(session),
        source: Some(binding.clone()),
        project_identity: Some(project_identity),
        import_source: Some(imported.clone()),
        import_identity: Some(import_identity),
        ..State::default()
    };
    let (input, renamed, dest) = (imported.clone(), moved.clone(), project.clone());
    crate::commands::native_save::inject_at_checkpoint(
        move || {
            fs::rename(input, &renamed).unwrap();
            fs::remove_file(&dest).unwrap();
            fs::hard_link(renamed, dest).unwrap();
        },
        || {
            let reply = state.save(project.clone(), true).unwrap();
            assert_eq!(reply.is_error, Some(true));
            let rmcp::model::ContentBlock::Text(text) = &reply.content[0] else {
                panic!("file receipt")
            };
            let receipt: serde_json::Value = serde_json::from_str(&text.text).unwrap();
            assert_eq!(receipt["operation"], "save");
            assert_eq!(receipt["replacement"], "not-replaced");
            assert!(receipt["staged_sha256"].as_str().is_some());
            assert!(receipt["acknowledged_state_id"].is_null());
            assert_eq!(receipt["durability"], "not-confirmed");
            assert!(
                receipt["reasons"]
                    .to_string()
                    .contains("captured imported image")
            );
        },
    );
    let session = state.session.as_ref().unwrap();
    assert!(session.is_dirty());
    assert_eq!(session.ticket(), ticket);
    assert_eq!(session.history_ticket(), history);
    assert_eq!(state.source.as_ref(), Some(&binding));
    assert_eq!(fs::read(&project).unwrap(), bytes);
    assert_eq!(fs::read(&moved).unwrap(), bytes);
    // Restoring neither pathname nor the source digest cannot silently bless an alias.
    assert!(state.protect_import_sources(&project).is_err());
    fs::remove_dir_all(root).unwrap();
}

#[test]
fn fresh_save_rechecks_retained_native_project_identity_at_the_checkpoint() {
    let root = std::env::temp_dir().join(format!(
        "paintcraft-project-save-checkpoint-{}",
        uuid::Uuid::new_v4()
    ));
    fs::create_dir(&root).unwrap();
    let source = root.join("source.pcnd");
    let moved = root.join("moved.pcnd");
    let output = root.join("copy.pcnd");
    let doc = new_document(2, 1, None, 0, POLICY).unwrap();
    let bytes = pcnd::write(&doc).unwrap();
    fs::write(&source, &bytes).unwrap();
    fs::write(&output, &bytes).unwrap();
    let (_, sha, identity) = read_captured(&source, INPUT_MAX, "project").unwrap();
    let mut session = Session::new(doc);
    session.mark_unsaved();
    let ticket = session.ticket();
    let history = session.history_ticket();
    let binding = (source.clone(), sha);
    let mut state = State {
        session: Some(session),
        source: Some(binding.clone()),
        project_identity: Some(identity),
        ..State::default()
    };
    let (input, renamed, dest) = (source.clone(), moved.clone(), output.clone());
    crate::commands::native_save::inject_at_checkpoint(
        move || {
            fs::rename(input, &renamed).unwrap();
            fs::remove_file(&dest).unwrap();
            fs::hard_link(renamed, dest).unwrap();
        },
        || {
            let reply = state.save(output.clone(), true).unwrap();
            assert_eq!(reply.is_error, Some(true));
            let rmcp::model::ContentBlock::Text(text) = &reply.content[0] else {
                panic!("file receipt")
            };
            let receipt: serde_json::Value = serde_json::from_str(&text.text).unwrap();
            assert_eq!(receipt["replacement"], "not-replaced");
            assert!(receipt["staged_sha256"].as_str().is_some());
            assert!(
                receipt["reasons"]
                    .to_string()
                    .contains("captured source project")
            );
            assert!(receipt["acknowledged_state_id"].is_null());
        },
    );
    let session = state.session.as_ref().unwrap();
    assert!(session.is_dirty());
    assert_eq!(session.ticket(), ticket);
    assert_eq!(session.history_ticket(), history);
    assert_eq!(state.source.as_ref(), Some(&binding));
    assert_eq!(fs::read(output).unwrap(), bytes);
    assert_eq!(fs::read(moved).unwrap(), bytes);
    fs::remove_dir_all(root).unwrap();
}
