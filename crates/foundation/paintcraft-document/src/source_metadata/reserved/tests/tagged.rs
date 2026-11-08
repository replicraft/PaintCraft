use super::*;
#[test]
fn independent_nonzero_tag_loop_and_invalid_tag_bounds_are_prepaid_before_retention() {
    let data = json::parse(include_bytes!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/tests/reserved-source-metadata-fixtures/tagged.json"
    )))
    .unwrap();
    for row in data.get("cases").unwrap().as_array().unwrap() {
        let header = bytes(row, "header_hex");
        let profile = bytes(row, "profile_hex");
        assert_eq!(profile.len(), 148);
        let icc = SharedBudget::new(1 << 20);
        let reservation = icc.reserve_capacity(1024).unwrap();
        let source = SharedBudget::new(8192);
        let scope = source
            .reserve_capacity(8192)
            .unwrap()
            .scope(AllocationClass::PaintMetadata);
        let mut guard = scope.account();
        guard.resize(512).unwrap();
        let used = Cell::new(0u64);
        let checks = Cell::new(0u64);
        let spend = |n: u64| {
            let next = used
                .get()
                .checked_add(n)
                .filter(|n| *n <= 268435456)
                .ok_or_else(|| Error::new(ErrorCode::ResourceLimit, "original tag work ceiling"))?;
            used.set(next);
            Ok(())
        };
        let check = || {
            checks.set(checks.get() + 1);
            Ok(())
        };
        let reader = ReservedSourceMetadataReader::new(
            &reservation,
            &scope,
            &guard,
            8 << 20,
            &check,
            &spend,
        )
        .unwrap();
        {
            let result = reader.inspect(&header, &profile);
            assert_eq!(
                used.get(),
                row.get("inspect_work_constructor_included")
                    .unwrap()
                    .as_i64()
                    .unwrap() as u64
            );
            assert_eq!(
                checks.get(),
                row.get("checks").unwrap().as_i64().unwrap() as u64
            );
            assert_eq!(reservation.available(), 1024);
            if row.get("accept").unwrap().as_bool() == Some(true) {
                let recipe = result.unwrap();
                let metadata = recipe.materialize().unwrap();
                assert_eq!(metadata.profile_header().unwrap().tag_count, 1);
                assert_eq!(metadata.profile().unwrap(), profile);
                assert_eq!(metadata.wire_header().as_slice(), header);
                assert_eq!(
                    metadata.storage_usage().1,
                    if cfg!(target_pointer_width = "64") {
                        636
                    } else {
                        620
                    }
                );
                drop(metadata);
            } else {
                assert_eq!(result.unwrap_err().code, ErrorCode::InputSchema);
            }
        }
        assert_eq!(reservation.available(), 1024);
        drop(reader);
        assert_eq!(scope.footprint().live, 512);
        drop(guard);
        drop(scope);
        drop(reservation);
        assert_eq!(source.usage().reserved, 0);
        assert_eq!(icc.usage().reserved, 0);
    }
}
