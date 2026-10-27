use super::*;
use std::sync::atomic::{AtomicUsize, Ordering};
struct Scratch(PathBuf);
impl Scratch {
    fn new() -> Self {
        let path =
            std::env::temp_dir().join(format!("paintcraft-jxr-export-{}", uuid::Uuid::new_v4()));
        std::fs::create_dir(&path).unwrap();
        Self(path)
    }
}
impl Drop for Scratch {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}
#[test]
fn transparent_preparation_and_cancellation_leave_existing_destination_and_session_unchanged() {
    let root = Scratch::new();
    let dest = root.0.join("existing.jxr");
    let original = b"original destination";
    std::fs::write(&dest, original).unwrap();
    let mut session =
        Session::new(new_document(65, 3, Some(Rgba([7, 19, 53, 127])), 0, POLICY).unwrap());
    session.mark_unsaved();
    let state = session.document().state_id;
    let ticket = session.history_ticket();
    let dirty = session.is_dirty();
    let owner = codec_worker::owner();
    // Alpha refusal happens before helper discovery or subprocess execution.
    let (_, ok, _, _) = write_copy(
        session.document(),
        &dest,
        true,
        &[],
        None,
        owner,
        &|| Ok(owner),
        SharedBudget::new(512 << 20),
        &|_| Ok(()),
        Arc::new(|| Ok(())),
    )
    .unwrap();
    assert!(!ok);
    assert_eq!(std::fs::read(&dest).unwrap(), original);
    let calls = Arc::new(AtomicUsize::new(0));
    let observed = calls.clone();
    let check: jxr_export_job::Check = Arc::new(move || {
        if observed.fetch_add(1, Ordering::Relaxed) >= 3 {
            return Err(Error::new(
                ErrorCode::OperationCancelled,
                "preparation cancelled",
            ));
        }
        Ok(())
    });
    let (_, ok, _, _) = write_copy(
        session.document(),
        &dest,
        true,
        &[],
        None,
        owner,
        &|| Ok(owner),
        SharedBudget::new(512 << 20),
        &|_| Ok(()),
        check,
    )
    .unwrap();
    assert!(!ok);
    assert!(calls.load(Ordering::Relaxed) >= 4);
    assert_eq!(std::fs::read(&dest).unwrap(), original);
    assert_eq!(std::fs::read_dir(&root.0).unwrap().count(), 1);
    assert_eq!(session.document().state_id, state);
    assert_eq!(session.history_ticket(), ticket);
    assert_eq!(session.is_dirty(), dirty);
}
#[test]
fn cancelled_owner_and_source_guards_refuse_before_encoding() {
    let root = Scratch::new();
    let dest = root.0.join("existing.jxr");
    let original = b"untouched";
    std::fs::write(&dest, original).unwrap();
    let doc = new_document(3, 2, Some(Rgba([7, 19, 53, 255])), 0, POLICY).unwrap();
    let owner = codec_worker::owner();
    let error = write_copy(
        &doc,
        &dest,
        true,
        &[],
        None,
        owner,
        &|| Ok(owner),
        SharedBudget::new(512 << 20),
        &|_| Ok(()),
        Arc::new(|| Err(Error::new(ErrorCode::OperationCancelled, "before stage"))),
    )
    .unwrap_err();
    assert_eq!(error.code, ErrorCode::OperationCancelled);
    let mut stale = owner;
    stale.epoch += 1;
    let (_, ok, _, _) = write_copy(
        &doc,
        &dest,
        true,
        &[],
        None,
        owner,
        &|| Ok(stale),
        SharedBudget::new(512 << 20),
        &|_| Ok(()),
        Arc::new(|| Ok(())),
    )
    .unwrap();
    assert!(!ok);
    let error = write_copy(
        &doc,
        &dest,
        true,
        &[],
        None,
        owner,
        &|| Ok(owner),
        SharedBudget::new(512 << 20),
        &|_| Err(Error::new(ErrorCode::InputSchema, "source alias")),
        Arc::new(|| Ok(())),
    )
    .unwrap_err();
    assert_eq!(error.code, ErrorCode::InputSchema);
    assert_eq!(std::fs::read(&dest).unwrap(), original);
    assert_eq!(std::fs::read_dir(&root.0).unwrap().count(), 1);
}
