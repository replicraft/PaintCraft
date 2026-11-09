use super::*;
fn planned() -> PortableLayerConfig {
    config(262144, 1, 1024)
}
fn m_for_plan() -> u64 {
    if cfg!(target_pointer_width = "64") {
        296976
    } else {
        247824
    }
}
#[test]
fn one_actual_tile_in_large_planned_table_prepays_retained_scan_before_late_cancel() {
    let (mut a, _q, res, _c) = owner(2 * T, 2 * PIXELS);
    let (_b, sc, _sr, m) = scratch(2 << 20, m_for_plan());
    let quote = RasterLayer::fresh_decode_index_quote(1024).unwrap();
    assert_eq!(quote.allocation_upper, 51216);
    assert_eq!(quote.bucket_upper, 2048);
    let used = Cell::new(0u64);
    let calls = Cell::new(0u64);
    let trace = RefCell::new(Vec::new());
    let late = Cell::new(false);
    let sentinel = Error::new(
        ErrorCode::OperationCancelled,
        "original planned index late cut",
    )
    .at(83);
    let spend = |n| {
        used.set(
            used.get()
                .checked_add(n)
                .filter(|v| *v <= WORK)
                .ok_or_else(limit)?,
        );
        trace.borrow_mut().push(n);
        Ok(())
    };
    let check = || {
        calls.set(calls.get() + 1);
        if late.get() {
            Err(sentinel.clone())
        } else {
            Ok(())
        }
    };
    let data = vec![7u8; 1024];
    let first = a
        .portable_layer_builder_checked(planned(), &sc, &m, &check, &spend)
        .unwrap()
        .block(
            0,
            0,
            EncodedTilePayload::new(PortablePixelEncoding::Raw, &data),
        )
        .unwrap();
    assert_eq!(res.available(), T);
    late.set(true);
    assert_eq!(
        first
            .block(
                1,
                0,
                EncodedTilePayload::new(PortablePixelEncoding::Raw, &data)
            )
            .unwrap_err(),
        sentinel
    );
    assert_eq!(
        used.get(),
        if cfg!(target_pointer_width = "64") {
            467456
        } else {
            467424
        }
    );
    assert_eq!(trace.borrow().iter().filter(|n| **n == 2112).count(), 1);
    assert_eq!(sc.footprint().live, 0);
    assert_eq!(res.available(), 2 * T);
    assert_eq!(a.remaining_pixels(), PIXELS);
}
#[test]
fn first_retirement_debit_refuses_before_decode_and_retry_keeps_original_paid_prefix() {
    let (mut a, _q, res, _c) = owner(2 * T, 2 * PIXELS);
    let (_b, sc, _sr, m) = scratch(2 << 20, m_for_plan());
    let used = Cell::new(0u64);
    let refusing = Cell::new(true);
    let sentinel = Error::new(ErrorCode::ResourceLimit, "original index retirement debit").at(84);
    let spend = |n| {
        if refusing.get() && n == 2112 {
            return Err(sentinel.clone());
        }
        used.set(
            used.get()
                .checked_add(n)
                .filter(|v| *v <= WORK)
                .ok_or_else(limit)?,
        );
        Ok(())
    };
    let check = || Ok(());
    let data = vec![7u8; 1024];
    let b = a
        .portable_layer_builder_checked(planned(), &sc, &m, &check, &spend)
        .unwrap();
    assert_eq!(
        b.block(
            0,
            0,
            EncodedTilePayload::new(PortablePixelEncoding::Raw, &data)
        )
        .unwrap_err(),
        sentinel
    );
    assert_eq!(used.get(), 55552);
    assert_eq!(a.remaining_pixels(), 2 * PIXELS);
    assert_eq!(res.available(), 2 * T);
    assert_eq!(sc.footprint().live, 0);
    refusing.set(false);
    let b = a
        .portable_layer_builder_checked(planned(), &sc, &m, &check, &spend)
        .unwrap()
        .block(
            0,
            0,
            EncodedTilePayload::new(PortablePixelEncoding::Raw, &data),
        )
        .unwrap();
    assert_eq!(b.finish().unwrap_err().code, ErrorCode::CodecFailed);
    assert_eq!(
        used.get(),
        55552
            + if cfg!(target_pointer_width = "64") {
                467424
            } else {
                467392
            }
    );
    assert_eq!(a.remaining_pixels(), PIXELS);
    assert_eq!(res.available(), 2 * T);
    assert_eq!(sc.footprint().live, 0);
}
