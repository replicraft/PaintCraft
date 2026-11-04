use super::*;
use paintcraft_core::types::resources::AllocationClass;
use paintcraft_core::types::{ErrorCode, json, text::MaskSpan};
use std::cell::Cell;

fn free_spend(_: u64) -> Result<()> {
    Ok(())
}
fn ok() -> Result<()> {
    Ok(())
}
fn rect(x: i64, y: i64, w: i64, h: i64) -> Rect {
    Rect::new(x, y, w, h).unwrap()
}
fn decode_hex(s: &str) -> Vec<u8> {
    s.as_bytes()
        .as_chunks::<2>()
        .0
        .iter()
        .map(|p| u8::from_str_radix(std::str::from_utf8(p).unwrap(), 16).unwrap())
        .collect()
}
fn vectors() -> json::Value {
    json::parse(include_bytes!(
        "../../../../tests/selection-snapshot-fixtures/vectors.json"
    ))
    .unwrap()
}
fn wire(name: &str) -> Vec<u8> {
    let vectors = vectors();
    let row = vectors
        .get("vectors")
        .unwrap()
        .as_array()
        .unwrap()
        .iter()
        .find(|v| v.get("name").unwrap().as_str() == Some(name))
        .unwrap();
    decode_hex(row.get("bytes_hex").unwrap().as_str().unwrap())
}
fn encode(mask: &BinaryMask, extent: Rect, scope: &BudgetScope, chunk: usize) -> Vec<u8> {
    let mut cursor = mask
        .snapshot_cursor(extent, scope, SnapshotLimits::default(), &ok, &free_spend)
        .unwrap();
    let mut output = vec![0; HEADER_BYTES];
    let mut buffer = vec![0; chunk];
    loop {
        let progress = cursor.step(&mut buffer, 131_072).unwrap();
        let (written, complete) = match progress {
            SnapshotProgress::Pending { written } => (written, false),
            SnapshotProgress::Complete { written } => (written, true),
        };
        output.extend_from_slice(&buffer[..written]);
        if complete {
            break;
        }
    }
    output[..HEADER_BYTES].copy_from_slice(&cursor.finish_header().unwrap());
    output
}
fn decode(bytes: &[u8], scope: &BudgetScope) -> DecodedSnapshot {
    let mut decoder =
        SnapshotDecoder::new(bytes, scope, SnapshotLimits::default(), &ok, &free_spend).unwrap();
    while matches!(
        decoder.step(131_072).unwrap(),
        SnapshotProgress::Pending { .. }
    ) {}
    decoder.finish().unwrap()
}
#[test]
fn weighted_selection_capture_preserves_fractional_samples_in_small_chunks() {
    let budget = SharedBudget::new(16 << 20);
    let scope = budget.scope(AllocationClass::SelectionMasks);
    let domain = rect(-2, -1, 4, 2);
    let values = [0, 1, 128, 255, 255, 63, 254, 0];
    let mask = BinaryMask::from_values(domain, &values, &budget, &ok)
        .unwrap()
        .inverted()
        .translated(3, 2)
        .unwrap();
    let bytes = encode(&mask, mask.domain(), &scope, 3);
    let decoded = decode(&bytes, &scope);
    let CapturedSelection::Field(field) = decoded.selection else {
        panic!("captured weighted field")
    };
    for y in mask.domain().y0..mask.domain().y1 {
        for x in mask.domain().x0..mask.domain().x1 {
            assert_eq!(field.coverage(x, y), mask.coverage(x, y));
        }
    }
}
fn sparse(
    domain: Rect,
    default: bool,
    samples: &[(i64, i64, u8)],
    budget: &SharedBudget,
) -> BinaryMask {
    let spans: Vec<_> = samples
        .iter()
        .map(|&(x, y, coverage)| MaskSpan {
            y,
            x0: x,
            x1: x,
            coverage,
        })
        .collect();
    BinaryMask::from_spans(domain, default, &spans, budget, MaskLimits::default(), &ok).unwrap()
}
#[test]
fn six_frozen_literal_decoders_match_independent_logical_rows_and_header_extents() {
    let vectors = vectors();
    for row in vectors.get("vectors").unwrap().as_array().unwrap() {
        let bytes = decode_hex(row.get("bytes_hex").unwrap().as_str().unwrap());
        let budget = SharedBudget::new(2 << 20);
        let scope = budget.scope(AllocationClass::SelectionMasks);
        let result = decode(&bytes, &scope);
        let extent: Vec<_> = row
            .get("extent")
            .unwrap()
            .as_array()
            .unwrap()
            .iter()
            .map(|n| n.as_i64().unwrap())
            .collect();
        assert_eq!(
            result.header.layer_extent,
            Rect {
                x0: extent[0],
                y0: extent[1],
                x1: extent[2],
                y1: extent[3]
            }
        );
        for (dy, expected) in row
            .get("expected_logical_rows")
            .unwrap()
            .as_array()
            .unwrap()
            .iter()
            .enumerate()
        {
            for (dx, expected) in expected.as_array().unwrap().iter().enumerate() {
                let value = match &result.selection {
                    CapturedSelection::Empty => 0,
                    CapturedSelection::Field(mask) => {
                        mask.coverage(extent[0] + dx as i64, extent[1] + dy as i64)
                    }
                };
                assert_eq!(i64::from(value), expected.as_i64().unwrap());
            }
        }
        drop(result);
        assert_eq!(scope.footprint().live, 0);
    }
}
#[test]
fn cursor_real_builder_sources_match_all_six_independent_literal_packets() {
    let budget = SharedBudget::new(16 << 20);
    let scope = budget.scope(AllocationClass::SelectionMasks);
    let uniform = BinaryMask::uniform(rect(0, 0, 3, 1), false).unwrap();
    let empty = BinaryMask::uniform(rect(10, 0, 1, 1), true).unwrap();
    let dense = sparse(
        rect(0, 0, 256, 256),
        false,
        &[(0, 0, 255), (2, 0, 255), (1, 1, 255)],
        &budget,
    );
    let rows: Vec<_> = (0..256)
        .map(|y| MaskSpan {
            y,
            x0: 0,
            x1: 255,
            coverage: 255,
        })
        .collect();
    let solid = BinaryMask::from_spans(
        rect(0, 0, 512, 256),
        false,
        &rows,
        &budget,
        MaskLimits::default(),
        &ok,
    )
    .unwrap();
    let negative = sparse(
        rect(-2, -1, 3, 2),
        false,
        &[(-2, -1, 255), (0, -1, 255), (-1, 0, 255)],
        &budget,
    )
    .translated(1, 0)
    .unwrap();
    let inverted = sparse(rect(0, 0, 3, 1), true, &[(1, 0, 0)], &budget).inverted();
    for (name, source, extent) in [
        ("uniform-zero", &uniform, rect(0, 0, 3, 1)),
        ("empty-intersection", &empty, rect(0, 0, 3, 1)),
        ("solid", &solid, rect(0, 0, 3, 1)),
        ("dense-lsb-partial", &dense, rect(0, 0, 3, 2)),
        ("negative-grid-offset-clip", &negative, rect(-2, -1, 4, 2)),
        ("default255-inverted", &inverted, rect(0, 0, 3, 1)),
    ] {
        for chunk in [1, 7, 31, 64] {
            assert_eq!(encode(source, extent, &scope, chunk), wire(name));
        }
    }
}
#[test]
fn huge_uniform_and_zero_record_packet_skip_grid_and_irrelevant_offset_arithmetic() {
    let domain = rect(-1_000_000, -1_000_000, 2_000_000, 2_000_000);
    let mask = BinaryMask::uniform(domain, true).unwrap();
    let budget = SharedBudget::new(4096);
    let scope = budget.scope(AllocationClass::SelectionMasks);
    let quote = mask
        .snapshot_quote(
            domain,
            SnapshotLimits {
                grid_cells: 0,
                ..SnapshotLimits::default()
            },
        )
        .unwrap();
    assert_eq!(quote.grid_cells, 0);
    let mut bytes = encode(&mask, domain, &scope, 8);
    bytes[88..96].copy_from_slice(&i64::MIN.to_be_bytes());
    let result = decode(&bytes, &scope);
    let CapturedSelection::Field(field) = result.selection else {
        panic!("active field")
    };
    assert_eq!(field.uniform_coverage(), Some(255));
    // Sampling BinaryMask with irrelevant offset can overflow: normalize the
    // no-storage decoder representation before exposing it (regression contract).
    assert_eq!(field.coverage(0, 0), 255);
}
#[test]
fn hostile_header_records_order_lengths_padding_and_trailing_bytes_are_refused() {
    let valid = wire("dense-lsb-partial");
    for at in [
        0,
        20,
        104,
        105,
        106,
        107,
        128 + 16,
        128 + 18,
        128 + 20,
        128 + 24,
        128 + 28,
    ] {
        let mut bytes = valid.clone();
        bytes[at] = 0x7f;
        let budget = SharedBudget::new(2 << 20);
        let scope = budget.scope(AllocationClass::SelectionMasks);
        let result =
            SnapshotDecoder::new(&bytes, &scope, SnapshotLimits::default(), &ok, &free_spend);
        if let Ok(mut decoder) = result {
            assert!(decoder.step(131_072).is_err());
        }
    }
    let mut padding = valid.clone();
    *padding.last_mut().unwrap() |= 0x80;
    let mut trailing = valid.clone();
    trailing.push(0);
    let len = trailing.len() as u64;
    trailing[8..16].copy_from_slice(&len.to_be_bytes());
    let original = wire("negative-grid-offset-clip");
    let mut duplicate = original[..128].to_vec();
    duplicate.extend_from_slice(&original[128..161]);
    duplicate.extend_from_slice(&original[128..161]);
    let len = duplicate.len() as u64;
    duplicate[8..16].copy_from_slice(&len.to_be_bytes());
    duplicate[16..20].copy_from_slice(&2u32.to_be_bytes());
    let mut unordered = original[..128].to_vec();
    unordered.extend_from_slice(&original[193..226]);
    unordered.extend_from_slice(&original[161..193]);
    unordered.extend_from_slice(&original[128..161]);
    for bytes in [padding, trailing, duplicate, unordered] {
        let budget = SharedBudget::new(2 << 20);
        let scope = budget.scope(AllocationClass::SelectionMasks);
        let mut decoder =
            SnapshotDecoder::new(&bytes, &scope, SnapshotLimits::default(), &ok, &free_spend)
                .unwrap();
        assert!(decoder.step(131_072).is_err());
        assert!(decoder.finish().is_err());
        assert_eq!(scope.footprint().live, 0);
    }
}
#[test]
fn exact_reserved_scope_denies_dense_init_without_borrowing_global_spare_capacity() {
    let bytes = wire("dense-lsb-partial");
    let global = SharedBudget::new(8 << 20);
    let scope = global
        .reserve_capacity(4096)
        .unwrap()
        .scope(AllocationClass::SelectionMasks);
    let mut decoder =
        SnapshotDecoder::new(&bytes, &scope, SnapshotLimits::default(), &ok, &free_spend).unwrap();
    assert!(decoder.step(131_072).is_err());
    assert!(decoder.finish().is_err());
    assert_eq!(scope.footprint().live, 0);
    assert_eq!(global.usage().reserved, 4096);
    drop(scope);
    assert_eq!(global.usage().reserved, 0);
}
#[test]
fn all_decoder_callback_failures_poison_and_release_private_storage() {
    let bytes = wire("dense-lsb-partial");
    let callbacks = Cell::new(0);
    {
        let budget = SharedBudget::new(2 << 20);
        let scope = budget.scope(AllocationClass::SelectionMasks);
        let check = || {
            callbacks.set(callbacks.get() + 1);
            Ok(())
        };
        let mut decoder = SnapshotDecoder::new(
            &bytes,
            &scope,
            SnapshotLimits::default(),
            &check,
            &free_spend,
        )
        .unwrap();
        while matches!(
            decoder.step(131_072).unwrap(),
            SnapshotProgress::Pending { .. }
        ) {}
        drop(decoder.finish().unwrap());
    }
    for fail in 1..=callbacks.get() {
        let budget = SharedBudget::new(2 << 20);
        let scope = budget.scope(AllocationClass::SelectionMasks);
        let calls = Cell::new(0);
        let check = || {
            calls.set(calls.get() + 1);
            if calls.get() == fail {
                Err(Error::new(
                    ErrorCode::OperationCancelled,
                    "fixture cancellation",
                ))
            } else {
                Ok(())
            }
        };
        if let Ok(mut decoder) = SnapshotDecoder::new(
            &bytes,
            &scope,
            SnapshotLimits::default(),
            &check,
            &free_spend,
        ) {
            if let Ok(progress) = decoder.step(131_072) {
                assert!(matches!(progress, SnapshotProgress::Complete { .. }));
            }
            assert!(decoder.finish().is_err());
        }
        assert_eq!(scope.footprint().live, 0);
    }
}
#[test]
fn cursor_callbacks_budget_and_quantum_preserve_source_and_never_reset_work() {
    let input = SharedBudget::new(2 << 20);
    let mask = sparse(rect(0, 0, 256, 256), false, &[(0, 0, 255)], &input);
    let budget = SharedBudget::new(4096);
    let scope = budget.scope(AllocationClass::SelectionMasks);
    let spent = Cell::new(0u64);
    let spend = |n| {
        spent.set(spent.get() + n);
        Ok(())
    };
    let cancelled = Cell::new(false);
    let check = || {
        if cancelled.get() {
            Err(Error::new(ErrorCode::OperationCancelled, "cancel"))
        } else {
            Ok(())
        }
    };
    let mut cursor = mask
        .snapshot_cursor(
            rect(0, 0, 3, 1),
            &scope,
            SnapshotLimits::default(),
            &check,
            &spend,
        )
        .unwrap();
    let mut output = [0u8; 1];
    assert_eq!(
        cursor.step(&mut output, 0).unwrap(),
        SnapshotProgress::Pending { written: 0 }
    );
    assert_eq!(spent.get(), HEADER_WORK);
    cancelled.set(true);
    assert!(cursor.step(&mut output, 131_072).is_err());
    assert!(cursor.finish_header().is_err());
    drop(cursor);
    assert_eq!(mask.coverage(0, 0), 255);
    assert_eq!(mask.coverage(1, 0), 0);
    assert_eq!(scope.footprint().live, 0);
    assert!(
        mask.snapshot_quote(
            rect(0, 0, 256, 256),
            SnapshotLimits {
                grid_cells: 0,
                ..SnapshotLimits::default()
            }
        )
        .is_err()
    );
}

#[test]
fn cursor_all_check_failures_and_external_work_denials_discard_partial_output() {
    let input = SharedBudget::new(2 << 20);
    let mask = sparse(rect(0, 0, 256, 256), false, &[(0, 0, 255)], &input);
    let identity = mask.storage_usage();
    let budget = SharedBudget::new(4096);
    let scope = budget.scope(AllocationClass::SelectionMasks);
    let count = Cell::new(0);
    let check = || {
        count.set(count.get() + 1);
        Ok(())
    };
    let mut output = [0u8; 64];
    {
        let mut cursor = mask
            .snapshot_cursor(
                rect(0, 0, 3, 1),
                &scope,
                SnapshotLimits::default(),
                &check,
                &free_spend,
            )
            .unwrap();
        assert!(matches!(
            cursor.step(&mut output, 131_072).unwrap(),
            SnapshotProgress::Complete { .. }
        ));
        assert!(cursor.finish_header().is_ok());
    }
    for fail in 1..=count.get() {
        let calls = Cell::new(0);
        let check = || {
            calls.set(calls.get() + 1);
            if calls.get() == fail {
                Err(Error::new(ErrorCode::OperationCancelled, "cursor phase"))
            } else {
                Ok(())
            }
        };
        if let Ok(mut cursor) = mask.snapshot_cursor(
            rect(0, 0, 3, 1),
            &scope,
            SnapshotLimits::default(),
            &check,
            &free_spend,
        ) {
            if let Ok(progress) = cursor.step(&mut output, 131_072) {
                assert!(matches!(progress, SnapshotProgress::Complete { .. }));
            }
            assert!(cursor.finish_header().is_err());
        }
        assert_eq!(scope.footprint().live, 0);
    }
    for allowance in [0u64, 1023, 1024, 1791, 1792, 1900] {
        let remaining = Cell::new(allowance);
        let spend = |n| {
            remaining.set(remaining.get().checked_sub(n).ok_or_else(limit)?);
            Ok(())
        };
        if let Ok(mut cursor) = mask.snapshot_cursor(
            rect(0, 0, 3, 1),
            &scope,
            SnapshotLimits::default(),
            &ok,
            &spend,
        ) {
            assert!(cursor.step(&mut output, 131_072).is_err());
            assert!(cursor.finish_header().is_err());
        }
        assert_eq!(scope.footprint().live, 0);
    }
    assert_eq!(mask.storage_usage(), identity);
    assert_eq!(mask.coverage(0, 0), 255);
}

#[test]
fn safe_borrowed_cursor_survives_actual_pending_future_without_map_clone() {
    use std::future::Future;
    use std::task::{Context, Poll, Waker};
    let input = SharedBudget::new(2 << 20);
    let mask = sparse(rect(0, 0, 256, 256), false, &[(0, 0, 255)], &input);
    let budget = SharedBudget::new(4096);
    let scope = budget.scope(AllocationClass::SelectionMasks);
    let scope_ref = &scope;
    let mut future = Box::pin(async move {
        let mut cursor = mask
            .snapshot_cursor(
                rect(0, 0, 3, 1),
                scope_ref,
                SnapshotLimits::default(),
                &ok,
                &free_spend,
            )
            .unwrap();
        let mut first = true;
        std::future::poll_fn(|_| {
            if first {
                first = false;
                Poll::Pending
            } else {
                Poll::Ready(())
            }
        })
        .await;
        let mut bytes = [0; 64];
        assert!(matches!(
            cursor.step(&mut bytes, 131_072).unwrap(),
            SnapshotProgress::Complete { .. }
        ));
        assert!(cursor.finish_header().is_ok());
    });
    let mut context = Context::from_waker(Waker::noop());
    assert!(future.as_mut().poll(&mut context).is_pending());
    assert!(scope.footprint().live > 0);
    assert!(future.as_mut().poll(&mut context).is_ready());
    drop(future);
    assert_eq!(scope.footprint().live, 0);
}

#[test]
fn decoder_complete_work_and_actual_scope_peak_fit_target_quotes_without_refund() {
    for name in [
        "uniform-zero",
        "solid",
        "dense-lsb-partial",
        "negative-grid-offset-clip",
    ] {
        let bytes = wire(name);
        let global = SharedBudget::new(2 << 20);
        let scope = global
            .reserve_capacity(1 << 20)
            .unwrap()
            .scope(AllocationClass::SelectionMasks);
        let used = Cell::new(0u64);
        let spend = |n| {
            used.set(used.get().checked_add(n).ok_or_else(limit)?);
            Ok(())
        };
        let mut decoder =
            SnapshotDecoder::new(&bytes, &scope, SnapshotLimits::default(), &ok, &spend).unwrap();
        while matches!(
            decoder.step(131_072).unwrap(),
            SnapshotProgress::Pending { .. }
        ) {}
        let quote = decoder.quote().unwrap();
        assert!(used.get() <= quote.work_upper);
        assert!(scope.footprint().peak <= quote.allocation_peak);
        let used_before = used.get();
        let result = decoder.finish().unwrap();
        let retained = match &result.selection {
            CapturedSelection::Empty => 0,
            CapturedSelection::Field(mask) => mask.storage_usage().map_or(0, |(_, bytes)| bytes),
        };
        assert_eq!(retained, quote.retained_upper);
        assert_eq!(scope.footprint().live, retained);
        drop(result);
        assert_eq!(used.get(), used_before);
        assert_eq!(scope.footprint().live, 0);
        drop(scope);
        assert_eq!(global.usage().reserved, 0);
    }
}

#[test]
fn sparse_1080p_quote_accepts_disjoint_tile_bound_that_whole_clip_bound_refused() {
    let input = SharedBudget::new(2 << 20);
    let extent = rect(0, 0, 1920, 1080);
    let mask = sparse(extent, false, &[(0, 0, 255)], &input);
    let limits = SnapshotLimits::default();
    let quote = mask.snapshot_quote(extent, limits).unwrap();
    let cells = 8 * 5;
    assert_eq!(quote.grid_cells, cells);
    let old_samples = 1920u64 * 1080;
    let old_stream = RECORD_BYTES as u64 + old_samples.div_ceil(8);
    let old_work = HEADER_WORK
        + cells * (GRID_WORK + RECORD_WORK)
        + old_samples * SAMPLE_WORK
        + old_stream * BYTE_WORK;
    assert!(old_work > limits.work, "causal previous quote refusal");
    let samples = DENSE_SAMPLES as u64;
    let stream = RECORD_BYTES as u64 + samples / 8;
    assert_eq!(quote.bytes_upper, HEADER_BYTES as u64 + stream);
    assert_eq!(
        quote.work_upper,
        HEADER_WORK
            + cells * (GRID_WORK + RECORD_WORK)
            + samples * SAMPLE_WORK
            + stream * BYTE_WORK
    );
    assert!(quote.work_upper <= limits.work);
}

#[test]
fn decoder_zero_quantum_keeps_prefix_and_backing_unadvanced_without_work_reset() {
    let bytes = wire("dense-lsb-partial");
    let budget = SharedBudget::new(2 << 20);
    let scope = budget.scope(AllocationClass::SelectionMasks);
    let used = Cell::new(0u64);
    let spend = |n| {
        used.set(used.get() + n);
        Ok(())
    };
    let mut decoder =
        SnapshotDecoder::new(&bytes, &scope, SnapshotLimits::default(), &ok, &spend).unwrap();
    let before = scope.footprint();
    for _ in 0..4 {
        assert_eq!(
            decoder.step(0).unwrap(),
            SnapshotProgress::Pending { written: 0 }
        );
        assert!(decoder.quote().is_none());
        assert_eq!(used.get(), HEADER_WORK);
        assert_eq!(scope.footprint(), before);
    }
    while matches!(
        decoder.step(131_072).unwrap(),
        SnapshotProgress::Pending { .. }
    ) {}
    drop(decoder.finish().unwrap());
    assert_eq!(scope.footprint().live, 0);
}

#[test]
fn decoder_exact_quoted_headroom_progresses_and_cancellation_at_each_yield_is_fatal() {
    for name in ["solid", "dense-lsb-partial", "negative-grid-offset-clip"] {
        let bytes = wire(name);
        let original = bytes.clone();
        let records = u64::from(u32_at(&bytes, 16));
        let mut completed_steps = 0;
        {
            let budget = SharedBudget::new(2 << 20);
            let scope = budget.scope(AllocationClass::SelectionMasks);
            let used = Cell::new(0u64);
            let spend = |n| {
                used.set(used.get() + n);
                Ok(())
            };
            let mut decoder =
                SnapshotDecoder::new(&bytes, &scope, SnapshotLimits::default(), &ok, &spend)
                    .unwrap();
            assert!(matches!(
                decoder.step(records * RECORD_WORK).unwrap(),
                SnapshotProgress::Pending { .. }
            ));
            let quote = decoder.quote().unwrap();
            loop {
                let before = used.get();
                let progress = decoder.step(quote.indivisible_work).unwrap();
                completed_steps += 1;
                assert!(
                    used.get() > before,
                    "exact headroom must advance charged work"
                );
                assert!(completed_steps < 128, "finite fixture continuation");
                if matches!(progress, SnapshotProgress::Complete { .. }) {
                    break;
                }
            }
            assert!(used.get() <= quote.work_upper);
            drop(decoder.finish().unwrap());
            assert_eq!(scope.footprint().live, 0);
        }
        for cut in 0..=completed_steps {
            let budget = SharedBudget::new(2 << 20);
            let scope = budget.scope(AllocationClass::SelectionMasks);
            let used = Cell::new(0u64);
            let cancelled = Cell::new(false);
            let check = || {
                if cancelled.get() {
                    Err(Error::new(
                        ErrorCode::OperationCancelled,
                        "decoder continuation",
                    ))
                } else {
                    Ok(())
                }
            };
            let spend = |n| {
                used.set(used.get() + n);
                Ok(())
            };
            let mut decoder =
                SnapshotDecoder::new(&bytes, &scope, SnapshotLimits::default(), &check, &spend)
                    .unwrap();
            assert!(matches!(
                decoder.step(records * RECORD_WORK).unwrap(),
                SnapshotProgress::Pending { .. }
            ));
            let quantum = decoder.quote().unwrap().indivisible_work;
            for _ in 0..cut {
                decoder.step(quantum).unwrap();
            }
            let before = used.get();
            cancelled.set(true);
            assert!(decoder.step(quantum).is_err());
            assert!(decoder.finish().is_err());
            assert_eq!(used.get(), before);
            assert_eq!(scope.footprint().live, 0);
        }
        assert_eq!(bytes, original);
    }
}
