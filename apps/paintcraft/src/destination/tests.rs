use super::*;
use std::{fs, sync::mpsc, thread, time::Duration};
fn scratch() -> PathBuf {
    let p = std::env::temp_dir().join(format!("paintcraft-binding-{}", Uuid::new_v4()));
    fs::create_dir(&p).unwrap();
    fs::canonicalize(p).unwrap()
}
fn write(binding: &Binding, path: &Path, bytes: &[u8]) -> Outcome {
    binding.save(|expected| {
        save::save_with_expectation_and_preparation(
            path,
            true,
            1024,
            expected,
            |sink| {
                sink.write_all(bytes)
                    .map_err(|e| Error::new(ErrorCode::StorageFailed, e.to_string()))
            },
            |_| Ok(()),
        )
    })
}
#[test]
fn acquired_content_survives_external_change_and_new_associations_advance() {
    let dir = scratch();
    let path = dir.join("source.pcnd");
    fs::write(&path, b"opened").unwrap();
    let (_, binding) = acquire(&path).unwrap();
    assert!(write(&binding, &path, b"first").is_success());
    assert!(write(&binding, &path, b"second").is_success());
    fs::write(&path, b"foreign").unwrap();
    assert!(!write(&binding, &path, b"must not write").is_success());
    assert_eq!(fs::read(&path).unwrap(), b"foreign");
    let new = Binding::authorized();
    let other = dir.join("copy.pcnd");
    assert!(write(&new, &other, b"copy").is_success());
    fs::remove_file(&other).unwrap();
    assert!(!write(&new, &other, b"must not recreate").is_success());
    assert!(!other.exists());
    fs::remove_dir_all(dir).unwrap();
}
#[test]
fn unknown_outcome_blocks_writes_until_its_own_receipt_is_strengthened() {
    let dir = scratch();
    let path = dir.join("source.pcnd");
    let binding = Binding::authorized();
    let mut acknowledged = None;
    let uncertain = binding.save(|_| {
        let outcome = save::save(&path, b"first", true);
        assert!(outcome.is_success());
        acknowledged = Some(outcome.clone());
        // Simulate loss of the replacement acknowledgement at the coordinator boundary.
        let mut unknown = outcome;
        unknown.replacement = Replacement::Unknown;
        unknown.durable = false;
        unknown
    });
    assert_eq!(uncertain.replacement, Replacement::Unknown);
    assert!(
        !binding
            .save(|_| panic!("unresolved writes must not dispatch"))
            .is_success()
    );
    binding.reconcile(acknowledged.as_ref().unwrap());
    assert!(write(&binding, &path, b"second").is_success());
    binding.reconcile(acknowledged.as_ref().unwrap());
    assert!(
        write(&binding, &path, b"third").is_success(),
        "old receipt must not regress the expected hash"
    );
    assert_eq!(fs::read(&path).unwrap(), b"third");
    fs::remove_dir_all(dir).unwrap();
}
#[test]
fn panic_disables_the_binding_and_a_concurrent_attempt_cannot_enter() {
    let binding = Binding::authorized();
    let shared = binding.clone();
    let (entered, waiting) = mpsc::channel();
    let (release, gate) = mpsc::channel();
    let worker = thread::spawn(move || {
        shared.save(|_| {
            entered.send(()).unwrap();
            gate.recv_timeout(Duration::from_secs(5)).unwrap();
            save::not_attempted(&Error::new(ErrorCode::StorageFailed, "before replacement"))
        })
    });
    waiting.recv_timeout(Duration::from_secs(5)).unwrap();
    assert!(
        !binding
            .save(|_| panic!("busy binding must not dispatch"))
            .is_success()
    );
    release.send(()).unwrap();
    worker.join().unwrap();
    assert!(std::panic::catch_unwind(|| binding.save(|_| panic!("injected I/O panic"))).is_err());
    assert!(
        !binding
            .save(|_| panic!("panicked binding must remain disabled"))
            .is_success()
    );
}
