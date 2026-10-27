use super::*;
#[test]
fn both_leases_survive_staging_and_cancellation_or_late_alias_does_not_publish() {
    let root = std::env::temp_dir().join(format!("paintcraft-msp-copy-{}", uuid::Uuid::new_v4()));
    std::fs::create_dir(&root).unwrap();
    let doc = new_document(9, 3, Some(Rgba([255; 4])), 0, POLICY).unwrap();
    let source = root.join("source.pcnd");
    let bytes = pcnd::write(&doc).unwrap();
    std::fs::write(&source, &bytes).unwrap();
    let (_, _, identity) = read_captured(&source, INPUT_MAX, "MSP source").unwrap();
    let moved = root.join("moved.pcnd");
    std::fs::rename(&source, &moved).unwrap();
    let dest = root.join("copy.msp");
    std::fs::write(&dest, b"original destination").unwrap();
    let pool = SharedBudget::new(64 << 20);
    let cancelled = || {
        if pool.usage().reserved > 0 {
            Err(Error::new(
                ErrorCode::OperationCancelled,
                "cancel after first admission",
            ))
        } else {
            Ok(())
        }
    };
    let (_, ok, _, _) = write_copy(
        &doc,
        Version::V2,
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
    assert_eq!(std::fs::read(&dest).unwrap(), b"original destination");
    let calls = std::cell::Cell::new(0);
    let protect = |path: &Path| {
        calls.set(calls.get() + 1);
        if calls.get() == 3 {
            let layout = msp::RgbaLayout::new(Version::V2.backend(), 9, 3)?;
            assert!(pool.usage().reserved > export::msp::render_reservation_bytes(layout)?);
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
        Version::V2,
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
    assert_eq!(pool.usage().reserved, 0);
    assert_eq!(std::fs::read(&dest).unwrap(), bytes);
    assert_eq!(std::fs::read(&moved).unwrap(), bytes);
    assert_eq!(std::fs::read_dir(&root).unwrap().count(), 2);
    std::fs::remove_dir_all(root).unwrap();
}
