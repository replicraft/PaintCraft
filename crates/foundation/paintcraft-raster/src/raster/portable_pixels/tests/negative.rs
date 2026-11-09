use super::*;
#[test]
fn finite_geometry_duplicates_extra_missing_and_codec_fences_never_fallback() {
    for (kind, bytes, width) in [
        (PortablePixelEncoding::Raw, vec![0; 3], 1),
        (PortablePixelEncoding::Runs, vec![0; 7], 1),
        (PortablePixelEncoding::Runs, vec![0, 0, 7, 8, 9, 0], 2),
        (PortablePixelEncoding::Runs, vec![1, 0, 7, 8, 9, 0], 1),
        (PortablePixelEncoding::Lz4, vec![48, 7, 8, 9], 1),
        (PortablePixelEncoding::Lz4, vec![0, 0, 0], 1),
        (PortablePixelEncoding::RunsLz4, vec![0, 0, 0, 0, 0], 1),
        (PortablePixelEncoding::RunsLz4, vec![7, 0, 0, 0, 0], 1),
        (PortablePixelEncoding::RunsLz4, vec![12, 0, 0, 0, 0], 1),
        (PortablePixelEncoding::RunsLz4, vec![6, 0, 0], 1),
        (PortablePixelEncoding::RunsLz4, vec![6, 0, 0, 0], 1),
        (
            PortablePixelEncoding::RunsLz4,
            vec![6, 0, 0, 0, 80, 0, 0, 1, 2, 3],
            1,
        ),
    ] {
        let (mut a, _q, res, _c) = owner(T, PIXELS);
        let (_b, sc, _sr, m) = scratch(2 << 20, full_m());
        let e = render(
            &mut a,
            &sc,
            &m,
            config(width, 1, 1),
            EncodedTilePayload::new(kind, &bytes),
            &|| Ok(()),
            &|_| Ok(()),
        )
        .unwrap_err();
        assert_eq!(e.code, ErrorCode::CodecFailed);
        assert_eq!(a.remaining_pixels(), PIXELS);
        assert_eq!(res.available(), T);
        assert_eq!(sc.footprint().live, 0);
    }
    for cfg in [config(0, 1, 1), config(1, 0, 1), config(1, 1, 2)] {
        let (mut a, _q, res, _c) = owner(T, PIXELS);
        let (_b, sc, _sr, m) = scratch(2 << 20, full_m());
        assert_eq!(
            a.portable_layer_builder_checked(cfg, &sc, &m, &|| Ok(()), &|_| Ok(()))
                .unwrap_err()
                .code,
            ErrorCode::InputSchema
        );
        assert_eq!(res.available(), T);
    }
    for which in 0..4 {
        let (mut a, _q, res, _c) = owner(2 * T, 2 * PIXELS);
        let (_b, sc, _sr, m) = scratch(2 << 20, 116 + 2 * (full_m() - 116));
        let first = vec![0; 1024];
        let tiny = [0; 4];
        let check = || Ok(());
        let spend = |_| Ok(());
        let b = a
            .portable_layer_builder_checked(
                config(257, 1, if which == 1 { 1 } else { 2 }),
                &sc,
                &m,
                &check,
                &spend,
            )
            .unwrap();
        let result = match which {
            0 => b
                .block(
                    0,
                    0,
                    EncodedTilePayload::new(PortablePixelEncoding::Raw, &first),
                )
                .unwrap()
                .block(
                    0,
                    0,
                    EncodedTilePayload::new(PortablePixelEncoding::Raw, &first),
                ),
            1 => b
                .block(
                    0,
                    0,
                    EncodedTilePayload::new(PortablePixelEncoding::Raw, &first),
                )
                .unwrap()
                .block(
                    1,
                    0,
                    EncodedTilePayload::new(PortablePixelEncoding::Raw, &tiny),
                ),
            2 => b.block(
                2,
                0,
                EncodedTilePayload::new(PortablePixelEncoding::Raw, &tiny),
            ),
            _ => {
                assert_eq!(b.finish().unwrap_err().code, ErrorCode::CodecFailed);
                assert_eq!(res.available(), 2 * T);
                continue;
            }
        };
        assert_eq!(result.unwrap_err().code, ErrorCode::CodecFailed);
        assert_eq!(res.available(), 2 * T);
        assert_eq!(sc.footprint().live, 0);
    }
}
#[test]
fn zero_tile_and_final_weak_alias_keep_original_q_controls_and_m_order() {
    let (mut adm, q, res, controls) = owner(T, PIXELS);
    let (_b, sc, _sr, m) = scratch(2 << 20, full_m());
    let mut cfg = config(1, 1, 1);
    cfg.default = Rgba::TRANSPARENT;
    let raster = render(
        &mut adm,
        &sc,
        &m,
        cfg,
        EncodedTilePayload::new(PortablePixelEncoding::Raw, &[0; 4]),
        &|| Ok(()),
        &|_| Ok(()),
    )
    .unwrap();
    assert_eq!(raster.resident_tiles(), 1);
    assert_eq!(res.available(), 0);
    assert!(
        raster
            .tiles
            .get(&TileCoord { tx: 0, ty: 0 })
            .unwrap()
            .pixels
            .iter()
            .all(|b| *b == 0)
    );
    let lease = raster.tile_leases().next().unwrap().1;
    let weak = lease.version();
    let factory_control = ReservedDecodeAdmission::control_quote()
        .unwrap()
        .fixed_bytes;
    drop(raster);
    drop(adm);
    assert_eq!(controls.footprint().live, factory_control);
    assert!(weak.try_lease().is_some());
    drop(lease);
    assert_eq!(controls.footprint().live, 0);
    assert_eq!(res.available(), T);
    assert!(weak.try_lease().is_none());
    // Actual Weak header lifetime ends BEFORE its external full M declaration.
    assert_eq!(m.bytes(), full_m());
    drop(weak);
    drop(m);
    drop(res);
    assert_eq!(q.usage().reserved, 0);
}

#[test]
fn zero_records_preserve_default_without_any_tile_lease_or_index_allocation() {
    let (mut a, _q, res, _c) = owner(0, 0);
    let (_b, sc, _sr, m) = scratch(4096, 0);
    let used = Cell::new(0u64);
    let calls = Cell::new(0u64);
    let spend = |n| {
        used.set(used.get() + n);
        Ok(())
    };
    let check = || {
        calls.set(calls.get() + 1);
        Ok(())
    };
    let raster = a
        .portable_layer_builder_checked(config(1, 1, 0), &sc, &m, &check, &spend)
        .unwrap()
        .finish()
        .unwrap();
    assert_eq!(raster.resident_tiles(), 0);
    assert_eq!(raster.get(0, 0), Rgba([1, 2, 3, 255]));
    assert_eq!(used.get(), 1728);
    assert_eq!(calls.get(), 3);
    assert_eq!(res.available(), 0);
    assert_eq!(sc.footprint().live, 0);
}

#[test]
fn maximum_run_extent_is_initialized_safely_and_expands_without_d_zeroing() {
    let j = json::parse(include_bytes!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/tests/portable-pixel-fixtures/max-r.json"
    )))
    .unwrap();
    let data = unhex(j.get("stored_hex").unwrap().as_str().unwrap());
    let (mut a, _q, res, _c) = owner(T, PIXELS);
    let (_b, sc, _sr, m) = scratch(659456, full_m());
    let used = Cell::new(0u64);
    let calls = Cell::new(0u64);
    let spend = |n| {
        used.set(
            used.get()
                .checked_add(n)
                .filter(|v| *v <= WORK)
                .ok_or_else(limit)?,
        );
        Ok(())
    };
    let check = || {
        calls.set(calls.get() + 1);
        Ok(())
    };
    let r = render(
        &mut a,
        &sc,
        &m,
        config(256, 256, 1),
        EncodedTilePayload::new(PortablePixelEncoding::RunsLz4, &data),
        &check,
        &spend,
    )
    .unwrap();
    assert_eq!(
        used.get(),
        if cfg!(target_pointer_width = "64") {
            37366148
        } else {
            37366116
        }
    );
    assert_eq!(calls.get(), 65551);
    let tile = r.tiles.get(&TileCoord { tx: 0, ty: 0 }).unwrap();
    assert_eq!(
        hex(&Sha256::digest(&tile.pixels)),
        j.get("physical_sha256").unwrap().as_str().unwrap()
    );
    assert_eq!(sc.footprint().peak, 659456);
    assert_eq!(sc.footprint().live, 0);
    drop(r);
    assert_eq!(res.available(), T);
}

mod planned_index;
