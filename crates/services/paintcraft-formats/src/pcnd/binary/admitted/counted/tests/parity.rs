use super::*;
#[test]
fn codec_patterns_match_legacy_portable_without_inventing_compressed_oracle() {
    for pattern in 0..5 {
        let (width, height) = if pattern == 4 { (3, 1) } else { (257, 3) };
        let pixels: Vec<u8> = (0..width * height)
            .flat_map(|i| match pattern {
                0 => [
                    i as u8,
                    (i * 17) as u8,
                    91,
                    if i % 2 == 0 { 0 } else { 255 },
                ],
                1 => [(i % 7) as u8, 23, 71, 97],
                2 => [(i / 97) as u8, 42, 61, 255],
                3 => [19, 43, 71, 97],
                _ => {
                    if i < 2 {
                        [19, 43, 71, 97]
                    } else {
                        [23, 47, 79, 101]
                    }
                }
            })
            .collect();
        let source =
            paintcraft_document::import_image(width, height, &pixels, "portable-parity").unwrap();
        let budget = SharedBudget::new(4 << 20);
        let scope = budget.scope(AllocationClass::PaintMetadata);
        let check = || Ok(());
        let spend = |_| Ok(());
        let fact =
            count_portable_normal_snapshot_rooted_checked(&source, &scope, &check, &spend).unwrap();
        let output = fact.encode().unwrap();
        let mut legacy = Vec::new();
        super::super::super::write_portable_snapshot_admitted_checked(
            &source,
            &mut legacy,
            &scope,
            &check,
            &spend,
        )
        .unwrap();
        assert_eq!(output.as_bytes(), legacy);
        assert_eq!(fact.wire_len(), legacy.len() as u64);
        assert!(source.same_semantics(&read(output.as_bytes())));
        drop(output);
        drop(fact);
        assert_eq!(scope.footprint().live, 0);
        assert_eq!(budget.usage().reserved, 0);
    }
}
#[test]
fn compact_and_sparse_tables_keep_exact_count_and_real_debit_trace() {
    let (compact, expected) = fixture("v6-catalog-sort-order");
    let mut sparse = compact.clone();
    let layer = sparse.layers[&sparse.roots[0]].clone();
    for i in 0..4096 {
        let mut extra = layer.clone();
        extra.id = uuid::Uuid::from_u128(10000 + i);
        sparse.layers.insert(extra.id, extra);
    }
    for i in 0..4096 {
        sparse.layers.remove(&uuid::Uuid::from_u128(10000 + i));
    }
    assert!(sparse.layers.capacity() > compact.layers.capacity());
    let mut traces = Vec::new();
    for source in [&compact, &sparse] {
        let budget = SharedBudget::new(4 << 20);
        let scope = budget.scope(AllocationClass::PaintMetadata);
        let trace = RefCell::new(Vec::new());
        let spend = |n| {
            trace.borrow_mut().push(n);
            Ok(())
        };
        let check = || Ok(());
        let fact =
            count_portable_normal_snapshot_rooted_checked(source, &scope, &check, &spend).unwrap();
        let output = fact.encode().unwrap();
        assert_eq!(output.as_bytes(), expected);
        drop(output);
        drop(fact);
        traces.push(trace.into_inner());
        assert_eq!(budget.usage().reserved, 0);
    }
    assert_eq!(traces[0], traces[1]);
}
