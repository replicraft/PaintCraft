//! Default-size coverage on the existing host and original cumulative counter.
use super::*;

const WORK_LIMIT: u64 = 268_435_456;

fn scene(index: usize) -> (Vec<u8>, [u8; 4]) {
    let fixtures = json::parse(include_bytes!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/tests/source8-display-fixtures/default-canvas.json"
    )))
    .unwrap();
    let row = &fixtures.get("cases").unwrap().as_array().unwrap()[index];
    let pixel = row.get("pixel").unwrap().as_array().unwrap();
    (
        unhex(row.get("hex").unwrap().as_str().unwrap()),
        std::array::from_fn(|i| pixel[i].as_i64().unwrap() as u8),
    )
}

fn full_view() -> Source8DisplayPatch {
    Source8DisplayPatch {
        visible: Rect::new(0, 0, 1000, 486).unwrap(),
        level: 4,
        patch: Rect::new(0, 0, 63, 31).unwrap(),
    }
}

fn render(
    host: &Host,
    wire: &[u8],
    display: Source8DisplayPatch,
    check: &dyn Fn() -> Result<()>,
) -> Result<Source8RgbaImage> {
    host.run(|| {
        render_portable_source8_display_checked(
            wire,
            host.request(),
            host.realms(),
            &host.frame(),
            display,
            check,
        )
    })
}

#[test]
fn normal_default_after_partial_corner_fits_one_job_and_output_outlives_source() {
    let host = Host::new(false);
    let (wire, pixel) = scene(0);
    let view = full_view();
    // The last cell contains only 8 by 6 native samples, not a padded 16 by 16.
    let corner = Source8DisplayPatch {
        patch: Rect::new(62, 30, 1, 1).unwrap(),
        ..view
    };
    let image = render(&host, &wire, corner, &|| Ok(())).unwrap();
    assert_eq!((image.width(), image.height()), (1, 1));
    assert_eq!(image.rgba(), pixel);
    drop(image);
    host.retired();
    assert_eq!(host.o.footprint().live, 0);
    let before = host.used();
    assert!(before > 0);

    let image = render(&host, &wire, view, &|| Ok(())).unwrap();
    assert_eq!((image.width(), image.height()), (63, 31));
    assert_eq!(image.rgba().len(), 63 * 31 * 4);
    assert!(image.rgba().as_chunks::<4>().0.iter().all(|p| *p == pixel));
    assert!(host.used() > before && host.used() <= WORK_LIMIT);
    host.retired();
    let budget = host.budget.clone();
    drop(host);
    assert_eq!(budget.usage().reserved, 2 << 20);
    assert!(image.rgba().as_chunks::<4>().0.iter().all(|p| *p == pixel));
    drop(image);
    assert_eq!(budget.usage().reserved, 0);
}

#[test]
fn cancelled_default_preserves_error_and_retry_does_not_renew_work() {
    let host = Host::new(false);
    let (wire, _) = scene(0);
    let sentinel = Error::new(
        ErrorCode::OperationCancelled,
        "default canvas cancellation".repeat(41),
    )
    .at(41);
    let pointer = sentinel.message.as_ptr();
    let pending = RefCell::new(Some(sentinel));
    let check = || {
        if host.o.footprint().live > 0
            && host.f.footprint().live == 0
            && let Some(error) = pending.borrow_mut().take()
        {
            return Err(error);
        }
        Ok(())
    };
    let error = render(&host, &wire, full_view(), &check).unwrap_err();
    assert_eq!(error.code, ErrorCode::OperationCancelled);
    assert_eq!(error.message.as_ptr(), pointer);
    assert_eq!(error.operation_index, Some(41));
    assert!(pending.borrow().is_none());
    host.retired();
    assert_eq!(host.o.footprint().live, 0);
    let paid = host.used();
    assert!(paid > WORK_LIMIT / 2);

    let error = render(&host, &wire, full_view(), &|| Ok(())).unwrap_err();
    assert_eq!(error.code, ErrorCode::ResourceLimit);
    assert!(host.used() > paid && host.used() <= WORK_LIMIT);
    host.retired();
    assert_eq!(host.o.footprint().live, 0);
    let budget = host.budget.clone();
    drop(host);
    assert_eq!(budget.usage().reserved, 0);
}

#[test]
fn general_default_fits_original_limit_and_preserves_complete_pixels() {
    let host = Host::new(false);
    let (wire, pixel) = scene(1);
    let image = render(&host, &wire, full_view(), &|| Ok(())).unwrap();
    assert_eq!((image.width(), image.height()), (63, 31));
    assert_eq!(image.rgba().len(), 63 * 31 * 4);
    assert!(image.rgba().as_chunks::<4>().0.iter().all(|p| *p == pixel));
    assert!(host.used() >= 137_907_884 && host.used() <= WORK_LIMIT);
    host.retired();
    let budget = host.budget.clone();
    drop(host);
    assert_eq!(budget.usage().reserved, 2 << 20);
    assert!(image.rgba().as_chunks::<4>().0.iter().all(|p| *p == pixel));
    drop(image);
    assert_eq!(budget.usage().reserved, 0);
}

#[test]
fn empty_normal_background_then_general_default_preserves_fractional_alpha() {
    use paintcraft_core::types::{LayerBlendMode, Rgba, ids::deterministic_id};
    use paintcraft_formats::pcnd::count_portable_blend_snapshot_rooted_checked;

    let mut doc = paintcraft_document::new_document(
        1000,
        486,
        Some(Rgba([7, 9, 11, 0])),
        0,
        paintcraft_document::POLICY,
    )
    .unwrap();
    let mut top = doc.layers[&doc.roots[0]].clone();
    top.id = deterministic_id(&["studio-default-transparent-background"]);
    top.opacity = 128;
    top.blend_mode = LayerBlendMode::Multiply;
    top.raster = paintcraft_raster::RasterLayer::new(1000, 486, Rgba([20, 60, 100, 128]));
    doc.roots.push(top.id);
    doc.layers.insert(top.id, top);

    // The source is produced by the actual Main preparation writer, with its
    // separate original scope funding both counting and exact encoded storage.
    let main_budget = SharedBudget::new(4 << 20);
    let main_scope = main_budget
        .reserve_capacity(4 << 20)
        .unwrap()
        .scope(AllocationClass::PaintMetadata);
    let check = || Ok(());
    let spend = |_| Ok(());
    let counted =
        count_portable_blend_snapshot_rooted_checked(&doc, &main_scope, &check, &spend).unwrap();
    let source = counted.encode().unwrap();
    let host = Host::new(false);
    let image = render(&host, source.as_bytes(), full_view(), &check).unwrap();
    assert_eq!((image.width(), image.height()), (63, 31));
    // Filter/mask-free alpha: round(128 * 128 / 255) = 64. The transparent
    // background contributes neither its hidden RGB nor an alpha sample.
    assert!(
        image
            .rgba()
            .as_chunks::<4>()
            .0
            .iter()
            .all(|p| *p == [20, 60, 100, 64])
    );
    assert!(host.used() <= WORK_LIMIT);
    drop(image);
    host.retired();
    assert_eq!(host.o.footprint().live, 0);
    let worker_budget = host.budget.clone();
    drop(host);
    assert_eq!(worker_budget.usage().reserved, 0);
    drop(source);
    drop(counted);
    assert_eq!(main_scope.footprint().live, 0);
    drop(main_scope);
    assert_eq!(main_budget.usage().reserved, 0);
}
