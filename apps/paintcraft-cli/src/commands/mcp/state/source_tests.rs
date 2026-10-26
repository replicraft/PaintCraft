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
