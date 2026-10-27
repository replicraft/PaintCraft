use super::*;
#[test]
fn cancellation_stale_destination_and_actual_alias_preserve_files_and_drop_lease() {
    let folder = std::env::temp_dir().join(format!("paintcraft-dds-copy-{}", uuid::Uuid::new_v4()));
    std::fs::create_dir(&folder).unwrap();
    let dest = folder.join("result.dds");
    let original = b"old destination";
    std::fs::write(&dest, original).unwrap();
    let doc = new_document(3, 2, Some(Rgba([17, 51, 93, 255])), 0, POLICY).unwrap();
    let options = dds_options::Options::default().resolve(3, 2).unwrap();
    let pool = SharedBudget::new(64 << 20);
    let calls = std::cell::Cell::new(0);
    let cancelled = || {
        calls.set(calls.get() + 1);
        if calls.get() == 5 {
            Err(Error::new(
                ErrorCode::OperationCancelled,
                "cancel during preparation",
            ))
        } else {
            Ok(())
        }
    };
    let (_, ok, _, _) = write_copy(
        &doc,
        options,
        &dest,
        true,
        &[],
        None,
        pool.clone(),
        &|_| Ok(()),
        &cancelled,
    )
    .unwrap();
    assert!(!ok);
    assert_eq!(pool.usage().reserved, 0);
    assert!(pool.usage().peak > 0);
    assert_eq!(std::fs::read(&dest).unwrap(), original);
    // Cancellation after staging but before publication keeps the old destination.
    let cancelled = std::cell::Cell::new(false);
    let calls = std::cell::Cell::new(0);
    let protect = |_: &Path| {
        calls.set(calls.get() + 1);
        if calls.get() == 3 {
            cancelled.set(true);
        }
        Ok(())
    };
    let check = || {
        if cancelled.get() {
            Err(Error::new(
                ErrorCode::OperationCancelled,
                "late cancellation",
            ))
        } else {
            Ok(())
        }
    };
    let (_, ok, _, _) = write_copy(
        &doc,
        options,
        &dest,
        true,
        &[],
        None,
        pool.clone(),
        &protect,
        &check,
    )
    .unwrap();
    assert!(!ok);
    assert_eq!(std::fs::read(&dest).unwrap(), original);
    assert_eq!(pool.usage().reserved, 0);
    // A rejected admission also cleans the stage without rendering/codec output.
    let denied = SharedBudget::new(1);
    let (_, ok, _, _) = write_copy(
        &doc,
        options,
        &dest,
        true,
        &[],
        None,
        denied.clone(),
        &|_| Ok(()),
        &|| Ok(()),
    )
    .unwrap();
    assert!(!ok);
    assert_eq!(denied.usage().peak, 0);
    assert_eq!(std::fs::read(&dest).unwrap(), original);
    // Mutating the destination after encoding must fail its captured fingerprint.
    let calls = std::cell::Cell::new(0);
    let protect = |_: &Path| {
        calls.set(calls.get() + 1);
        if calls.get() == 3 {
            assert!(pool.usage().reserved > 0);
            std::fs::write(&dest, b"late destination replacement").unwrap();
        }
        Ok(())
    };
    let (_, ok, _, _) = write_copy(
        &doc,
        options,
        &dest,
        true,
        &[],
        None,
        pool.clone(),
        &protect,
        &|| Ok(()),
    )
    .unwrap();
    assert!(!ok);
    assert_eq!(
        std::fs::read(&dest).unwrap(),
        b"late destination replacement"
    );
    assert_eq!(pool.usage().reserved, 0);
    // Real renamed/hardlinked source identity, introduced only at publication.
    let source = folder.join("source.pcnd");
    let source_bytes = pcnd::write(&doc).unwrap();
    std::fs::write(&source, &source_bytes).unwrap();
    let (_, _, identity) = read_captured(&source, INPUT_MAX, "DDS source").unwrap();
    let moved = folder.join("moved.pcnd");
    std::fs::rename(&source, &moved).unwrap();
    let calls = std::cell::Cell::new(0);
    let protect = |path: &Path| {
        calls.set(calls.get() + 1);
        if calls.get() == 3 {
            std::fs::remove_file(&dest).unwrap();
            std::fs::hard_link(&moved, &dest).unwrap();
        }
        if identity.matches_path(path)? {
            return Err(Error::new(ErrorCode::InputSchema, "captured source alias"));
        }
        Ok(())
    };
    let (_, ok, _, _) = write_copy(
        &doc,
        options,
        &dest,
        true,
        &[],
        None,
        pool.clone(),
        &protect,
        &|| Ok(()),
    )
    .unwrap();
    assert!(!ok);
    assert_eq!(std::fs::read(&dest).unwrap(), source_bytes);
    assert_eq!(std::fs::read(&moved).unwrap(), source_bytes);
    assert_eq!(pool.usage().reserved, 0);
    assert_eq!(std::fs::read_dir(&folder).unwrap().count(), 2);
    std::fs::remove_dir_all(folder).unwrap();
}
