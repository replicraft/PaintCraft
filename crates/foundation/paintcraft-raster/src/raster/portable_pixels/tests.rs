use super::*;
use paintcraft_core::types::json::{self, Value};
use sha2::{Digest, Sha256};
use std::cell::{Cell, RefCell};
const T: u64 = 262144;
const PIXELS: u64 = 65536;
const WORK: u64 = 268435456;
fn target() -> &'static str {
    if cfg!(target_pointer_width = "64") {
        "native64"
    } else {
        "wasm32"
    }
}
fn full_m() -> u64 {
    if cfg!(target_pointer_width = "64") {
        356
    } else {
        308
    }
}
fn config(width: u32, height: u32, records: usize) -> PortableLayerConfig {
    PortableLayerConfig {
        width,
        height,
        default: Rgba([1, 2, 3, 255]),
        records,
    }
}
fn owner(
    cap: u64,
    pixels: u64,
) -> (
    ReservedDecodeAdmission,
    SharedBudget,
    BudgetReservation,
    BudgetScope,
) {
    let budget = SharedBudget::new(8 * T);
    let reservation = budget.reserve_capacity(cap).unwrap();
    let controls = SharedBudget::new(4096).scope(AllocationClass::PaintMetadata);
    let factories = ReservedDecodeFactories {
        selection_budget: SharedBudget::new(4096),
        coverage_budget: SharedBudget::new(4096),
        paint_metadata_scope: controls.clone(),
    };
    let a = Admission::with_reserved_decode_factories(
        pixels,
        reservation.clone(),
        factories,
        &controls,
        &|| Ok(()),
    )
    .unwrap();
    (a, budget, reservation, controls)
}
fn scratch(cap: u64, m: u64) -> (SharedBudget, BudgetScope, BudgetReservation, BudgetAccount) {
    let budget = SharedBudget::new(16 << 20);
    let reservation = budget.reserve_capacity(cap).unwrap();
    let scope = reservation.clone().scope(AllocationClass::PaintMetadata);
    let mut metadata = budget.account(AllocationClass::PaintMetadata);
    metadata.resize(m).unwrap();
    (budget, scope, reservation, metadata)
}
fn hex(bytes: &[u8]) -> String {
    bytes.iter().map(|b| format!("{b:02x}")).collect()
}
fn unhex(s: &str) -> Vec<u8> {
    s.as_bytes()
        .as_chunks::<2>()
        .0
        .iter()
        .map(|p| u8::from_str_radix(std::str::from_utf8(p).unwrap(), 16).unwrap())
        .collect()
}
fn fixture_rows() -> Vec<Value> {
    json::parse(include_bytes!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/tests/portable-pixel-fixtures/byte-oracles-first.json"
    )))
    .unwrap()
    .get("fixtures")
    .unwrap()
    .as_array()
    .unwrap()
    .to_vec()
}
fn encoding(s: &str) -> PortablePixelEncoding {
    match s {
        "raw" => PortablePixelEncoding::Raw,
        "runs" => PortablePixelEncoding::Runs,
        "lz4" => PortablePixelEncoding::Lz4,
        "runs-lz4" => PortablePixelEncoding::RunsLz4,
        _ => panic!("fixture"),
    }
}
fn render(
    a: &mut ReservedDecodeAdmission,
    sc: &BudgetScope,
    m: &BudgetAccount,
    cfg: PortableLayerConfig,
    payload: EncodedTilePayload<'_>,
    check: &dyn Fn() -> Result<()>,
    spend: &dyn Fn(u64) -> Result<()>,
) -> Result<RasterLayer> {
    a.portable_layer_builder_checked(cfg, sc, m, check, spend)?
        .block(0, 0, payload)?
        .finish()
}
fn events(kind: &str) -> Vec<(String, u64)> {
    let j = json::parse(include_bytes!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/tests/portable-pixel-fixtures/ledger-first.json"
    )))
    .unwrap();
    let mut events = vec![
        ("new_math".into(), 64),
        ("new_geometry_check".into(), 576),
        ("new_index_check".into(), 628),
        ("new_return".into(), 32),
    ];
    let rows = j
        .get("targets")
        .unwrap()
        .get(target())
        .unwrap()
        .get("vectors")
        .unwrap()
        .get(kind)
        .unwrap()
        .get("events")
        .unwrap()
        .as_array()
        .unwrap();
    for row in rows {
        let a = row.as_array().unwrap();
        events.push((a[0].as_str().unwrap().into(), a[1].as_i64().unwrap() as u64));
    }
    events.push(("finish_check".into(), 544));
    events
}
fn is_check(name: &str) -> bool {
    name.ends_with("check")
        || matches!(
            name,
            "geometry"
                | "entry_lookup_hash"
                | "scratchD_reserve"
                | "run_header"
                | "rle_exact"
                | "q_admission"
                | "copy_full"
                | "regions_revision"
                | "tile_arc"
                | "insert_destination"
                | "terminal"
        )
}
#[test]
fn defining_layouts_and_fixed_control_slot_match_frozen_targets() {
    let native = cfg!(target_pointer_width = "64");
    assert_eq!(
        size_of::<PortableLayerConfig>(),
        if native { 24 } else { 16 }
    );
    assert_eq!(
        size_of::<PortableLayerBuilder<'static>>(),
        if native { 208 } else { 144 }
    );
    assert_eq!(
        size_of::<EncodedTilePayload<'static>>(),
        if native { 24 } else { 12 }
    );
    assert_eq!(size_of::<PixelGeometry>(), if native { 56 } else { 48 });
    assert_eq!(
        size_of::<decode::FixedBytes>(),
        if native { 64 } else { 40 }
    );
    assert_eq!(size_of::<tile::PendingTile>(), if native { 88 } else { 48 });
    assert_eq!(size_of::<Meter<'static>>(), if native { 48 } else { 32 });
    assert!(
        size_of::<std::collections::hash_map::Entry<'static, TileCoord, Arc<Tile>>>()
            <= ENTRY_CONTROL_ENVELOPE
    );
    assert_eq!(control_sum().unwrap(), if native { 2584 } else { 2100 });
    assert_eq!(
        ReservedDecodeAdmission::control_quote()
            .unwrap()
            .tile_header_bytes,
        if native { 200 } else { 168 }
    );
    assert_eq!(
        BudgetLease::control_allocation_quote().unwrap(),
        if native { 40 } else { 24 }
    );
    assert_eq!(PORTABLE_PIXEL_CONTROL_BYTES, 4096);
}
#[test]
fn independent_codec_pixels_hidden_rgb_padding_and_complete_event_vectors() {
    for row in fixture_rows() {
        let bytes = unhex(row.get("stored_hex").unwrap().as_str().unwrap());
        let name = row.get("name").unwrap().as_str().unwrap();
        let full = row.get("decoded").unwrap().as_i64().unwrap() == T as i64;
        let cfg = if full {
            config(256, 256, 1)
        } else {
            config(1, 1, 1)
        };
        let (mut adm, q, res, controls) = owner(T, PIXELS);
        let (_b, sc, _sr, m) = scratch(2 << 20, full_m());
        let trace = RefCell::new(Vec::new());
        let used = Cell::new(0u64);
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
        let raster = render(
            &mut adm,
            &sc,
            &m,
            cfg,
            EncodedTilePayload::new(
                encoding(row.get("encoding").unwrap().as_str().unwrap()),
                &bytes,
            ),
            &|| Ok(()),
            &spend,
        )
        .unwrap();
        let tile = raster.tiles.get(&TileCoord { tx: 0, ty: 0 }).unwrap();
        assert_eq!(tile.pixels.len(), T as usize);
        assert_eq!(
            hex(&Sha256::digest(&tile.pixels)),
            row.get("physical_sha256").unwrap().as_str().unwrap()
        );
        assert_eq!(raster.resident_tiles(), 1);
        assert_eq!(res.available(), 0);
        let kind = match name {
            "raw-full" => Some("raw_full"),
            "raw-1x1" => Some("raw_1x1_padded"),
            "runs-full" => Some("runs_full_uniform"),
            "lz4-full" => Some("lz4_full_uniform_E1033"),
            "runs-lz4-full" => Some("runs_lz4_full_uniform_R6_E11"),
            _ => None,
        };
        if let Some(kind) = kind {
            assert_eq!(
                *trace.borrow(),
                events(kind).iter().map(|e| e.1).collect::<Vec<_>>()
            );
        }
        assert_eq!(sc.footprint().live, 0);
        drop(raster);
        assert_eq!(res.available(), T);
        drop(adm);
        assert_eq!(controls.footprint().live, 0);
        drop(res);
        assert_eq!(q.usage().reserved, 0);
    }
}
#[test]
fn zero_raw_still_owns_physical_tile_and_two_edge_blocks_have_correct_padding() {
    let (mut adm, _q, res, _controls) = owner(2 * T, 2 * PIXELS);
    let (_b, sc, _sr, m) = scratch(2 << 20, 2 * (full_m() - 116) + 116);
    let first = vec![7u8; 1024];
    let second = [9, 8, 7, 0];
    let r = adm
        .portable_layer_builder_checked(config(257, 1, 2), &sc, &m, &|| Ok(()), &|_| Ok(()))
        .unwrap()
        .block(
            0,
            0,
            EncodedTilePayload::new(PortablePixelEncoding::Raw, &first),
        )
        .unwrap()
        .block(
            1,
            0,
            EncodedTilePayload::new(PortablePixelEncoding::Raw, &second),
        )
        .unwrap()
        .finish()
        .unwrap();
    assert_eq!(r.get(256, 0), Rgba(second));
    assert_eq!(r.resident_tiles(), 2);
    let tile = r.tiles.get(&TileCoord { tx: 1, ty: 0 }).unwrap();
    assert_eq!(&tile.pixels[4..8], &[1, 2, 3, 255]);
    assert_eq!(&tile.pixels[1024..1028], &[1, 2, 3, 255]);
    drop(r);
    assert_eq!(res.available(), 2 * T);
    let mut cfg = config(1, 1, 1);
    cfg.default = Rgba::TRANSPARENT;
    let r = render(
        &mut adm,
        &sc,
        &m,
        cfg,
        EncodedTilePayload::new(PortablePixelEncoding::Raw, &[0; 4]),
        &|| Ok(()),
        &|_| Ok(()),
    );
    assert_eq!(r.unwrap_err().code, ErrorCode::ResourceLimit); // original staged pixels already consumed twice
}
#[test]
fn index_metadata_and_full_header_union_refuse_at_distinct_frozen_prefixes() {
    for (mbytes, expected) in [(115, 640), (full_m() - 1, 1876)] {
        let (mut adm, _q, res, _c) = owner(T, PIXELS);
        let (_b, sc, _sr, m) = scratch(2 << 20, mbytes);
        let used = Cell::new(0u64);
        let spend = |n| {
            used.set(used.get() + n);
            Ok(())
        };
        let error = render(
            &mut adm,
            &sc,
            &m,
            config(1, 1, 1),
            EncodedTilePayload::new(PortablePixelEncoding::Raw, &[0; 4]),
            &|| Ok(()),
            &spend,
        )
        .unwrap_err();
        assert_eq!(error.code, ErrorCode::ResourceLimit);
        assert_eq!(used.get(), expected);
        assert_eq!(adm.remaining_pixels(), PIXELS);
        assert_eq!(res.available(), T);
        assert_eq!(sc.footprint().live, 0);
    }
    let (mut adm, _q, _res, _c) = owner(T, PIXELS);
    let (_b, sc, _sr, m) = scratch(4095, full_m());
    let used = Cell::new(0u64);
    let spend = |n| {
        used.set(used.get() + n);
        Ok(())
    };
    let result = adm.portable_layer_builder_checked(config(1, 1, 1), &sc, &m, &|| Ok(()), &spend);
    assert_eq!(result.unwrap_err().code, ErrorCode::ResourceLimit);
    assert_eq!(used.get(), 64);
}
#[test]
fn scratch_and_original_q_denials_do_not_borrow_parent_headroom_or_refund_pixels() {
    for (kind, data, need) in [
        (PortablePixelEncoding::Runs, vec![255, 255, 7, 8, 9, 0], T),
        (
            PortablePixelEncoding::RunsLz4,
            vec![6, 0, 0, 0, 96, 255, 255, 7, 8, 9, 0],
            T + 6,
        ),
    ] {
        let (mut adm, _q, res, _c) = owner(T, PIXELS);
        let (_b, sc, _sr, m) = scratch(4096 + need - 1, full_m());
        let e = render(
            &mut adm,
            &sc,
            &m,
            config(256, 256, 1),
            EncodedTilePayload::new(kind, &data),
            &|| Ok(()),
            &|_| Ok(()),
        )
        .unwrap_err();
        assert_eq!(e.code, ErrorCode::ResourceLimit);
        assert_eq!(res.available(), T);
        assert_eq!(adm.remaining_pixels(), PIXELS);
        assert_eq!(sc.footprint().live, 0);
    }
    let (mut adm, _q, res, _c) = owner(T - 1, 2 * PIXELS);
    let (_b, sc, _sr, m) = scratch(2 << 20, full_m());
    let used = Cell::new(0u64);
    let spend = |n| {
        used.set(used.get() + n);
        Ok(())
    };
    for attempt in 1..=2 {
        let e = render(
            &mut adm,
            &sc,
            &m,
            config(256, 256, 1),
            EncodedTilePayload::new(PortablePixelEncoding::Runs, &[255, 255, 7, 8, 9, 0]),
            &|| Ok(()),
            &spend,
        )
        .unwrap_err();
        assert_eq!(e.code, ErrorCode::ResourceLimit);
        assert_eq!(adm.remaining_pixels(), (2 - attempt) * PIXELS);
        assert_eq!(res.available(), T - 1);
        assert_eq!(sc.footprint().live, 0);
        assert_eq!(used.get(), attempt * 529010);
    }
    assert!(used.get() > 0);
}
#[test]
fn all_raw_and_owned_run_callbacks_and_debits_preserve_first_error_and_paid_retry() {
    for (kind, encoding, data) in [
        (
            "raw_full",
            PortablePixelEncoding::Raw,
            vec![7u8; T as usize],
        ),
        (
            "runs_full_uniform",
            PortablePixelEncoding::Runs,
            vec![255, 255, 7, 8, 9, 0],
        ),
    ] {
        let expected = events(kind);
        let checks = expected.iter().filter(|e| is_check(&e.0)).count();
        for cancel in [false, true] {
            for stop in 0..if cancel { checks } else { expected.len() } {
                let (mut adm, _q, res, _c) = owner(2 * T, 2 * PIXELS);
                let (_b, sc, _sr, m) = scratch(2 << 20, full_m());
                let ordinal = Cell::new(0usize);
                let calls = Cell::new(0usize);
                let used = Cell::new(0u64);
                let refusing = Cell::new(true);
                let sentinel = Error::new(
                    if cancel {
                        ErrorCode::OperationCancelled
                    } else {
                        ErrorCode::ResourceLimit
                    },
                    "original pixel cut",
                )
                .at(81);
                let spend = |n| {
                    let i = ordinal.get();
                    ordinal.set(i + 1);
                    if !cancel && refusing.get() && i == stop {
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
                let check = || {
                    let i = calls.get();
                    calls.set(i + 1);
                    if cancel && refusing.get() && i == stop {
                        Err(sentinel.clone())
                    } else {
                        Ok(())
                    }
                };
                assert_eq!(
                    render(
                        &mut adm,
                        &sc,
                        &m,
                        config(256, 256, 1),
                        EncodedTilePayload::new(encoding, &data),
                        &check,
                        &spend
                    )
                    .unwrap_err(),
                    sentinel
                );
                let prefix = if cancel {
                    let i = expected
                        .iter()
                        .enumerate()
                        .filter(|(_, e)| is_check(&e.0))
                        .nth(stop)
                        .unwrap()
                        .0;
                    expected[..=i].iter().map(|e| e.1).sum::<u64>()
                } else {
                    expected[..stop].iter().map(|e| e.1).sum()
                };
                assert_eq!(used.get(), prefix);
                assert_eq!(sc.footprint().live, 0);
                assert_eq!(res.available(), 2 * T);
                refusing.set(false);
                let raster = render(
                    &mut adm,
                    &sc,
                    &m,
                    config(256, 256, 1),
                    EncodedTilePayload::new(encoding, &data),
                    &check,
                    &spend,
                )
                .unwrap();
                assert_eq!(
                    used.get(),
                    prefix + expected.iter().map(|e| e.1).sum::<u64>()
                );
                drop(raster);
                assert_eq!(res.available(), 2 * T);
            }
        }
    }
}

mod negative;
