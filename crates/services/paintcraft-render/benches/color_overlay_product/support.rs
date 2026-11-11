use paintcraft_core::types::{
    Rect, Rgba,
    filters::Descriptor,
    json::{self, Value},
    resources::{AllocationClass, SharedBudget},
    styles::{ColorOverlayRgbV1, EffectBlendMode},
};
use paintcraft_document::{
    Document, FilterInstance, FilterMask, FilterStack, LayerBlendMode, LayerMask, StyleDescriptor,
    StyleInstance, StyleStack,
};
use paintcraft_raster::{
    Admission, RasterLayer,
    coverage_mask::{A8Mask, Limits},
};
use paintcraft_render::spatial::WorkspaceAdmission;
use std::cell::Cell;
pub fn manifest() -> Value {
    json::parse(include_bytes!("cases.json")).unwrap()
}
pub fn number(row: &Value, key: &str) -> u64 {
    row.get(key).unwrap().as_i64().unwrap().try_into().unwrap()
}
pub fn expected(row: &Value, key: &str) -> u64 {
    u64::from_str_radix(row.get(key).unwrap().as_str().unwrap(), 16).unwrap()
}
pub fn hash_bytes(bytes: &[u8]) -> u64 {
    bytes.iter().fold(14695981039346656037_u64, |h, b| {
        (h ^ u64::from(*b)).wrapping_mul(1099511628211)
    })
}
pub fn hash(pixels: &[Rgba]) -> u64 {
    pixels
        .iter()
        .flat_map(|p| p.0)
        .fold(14695981039346656037_u64, |h, b| {
            (h ^ u64::from(b)).wrapping_mul(1099511628211)
        })
}
pub fn pixels(n: usize) -> Vec<Rgba> {
    (0..n)
        .map(|i| {
            Rgba([
                (i * 17 + 3) as u8,
                (i * 29 + 127) as u8,
                255_u8.wrapping_sub((i * 7) as u8),
                [0, 1, 127, 128, 254, 255][i % 6],
            ])
        })
        .collect()
}
pub fn policy(mode: u8, opacity: u8) -> ColorOverlayRgbV1 {
    ColorOverlayRgbV1::new(
        [220, 40, 0],
        opacity,
        EffectBlendMode::from_code(mode).unwrap(),
    )
}

pub fn document(row: &Value) -> Document {
    let w = number(row, "width") as u32;
    let h = number(row, "height") as u32;
    let mode = number(row, "mode") as u8;
    let full = row.get("full_pipeline").unwrap().as_bool().unwrap();
    let mut doc = paintcraft_document::new_document(
        w,
        h,
        Some(Rgba([20, 80, 200, 127])),
        0,
        paintcraft_document::POLICY,
    )
    .unwrap();
    let first = doc.roots[0];
    let mut top = doc.layers[&first].clone();
    top.styles = StyleStack::new(vec![
        StyleInstance::new(
            paintcraft_core::types::ids::parse_uuid("00000000-0000-0000-0000-00000000000e")
                .unwrap(),
            true,
            StyleDescriptor::ColorOverlayRgbV1(policy(mode, 128)),
        )
        .unwrap(),
    ])
    .unwrap();
    if full {
        doc.layers.get_mut(&first).unwrap().raster =
            RasterLayer::new(w, h, Rgba([31, 127, 241, 128]));
        top.id = paintcraft_core::types::ids::parse_uuid("00000000-0000-0000-0000-00000000000d")
            .unwrap();
        let input: Vec<u8> = pixels(w as usize * h as usize)
            .into_iter()
            .flat_map(|p| p.0)
            .collect();
        top.raster =
            RasterLayer::from_rgba(w, h, Rgba([0; 4]), &input, &mut Admission::new(8 << 20))
                .unwrap();
        let source = SharedBudget::new(1 << 20);
        let domain = Rect::new(0, 0, i64::from(w), i64::from(h)).unwrap();
        let coverage =
            A8Mask::uniform(domain, 127, &source, Limits::default(), &|| Ok(())).unwrap();
        top.filters = FilterStack::new(vec![
            FilterInstance::new(
                paintcraft_core::types::ids::parse_uuid("00000000-0000-0000-0000-00000000002c")
                    .unwrap(),
                true,
                Descriptor::InvertRgbV1,
            )
            .with_mask(Some(FilterMask::new(true, coverage.clone()))),
        ])
        .unwrap();
        top.mask = Some(LayerMask::new(true, coverage));
        top.opacity = 128;
        top.blend_mode = LayerBlendMode::Multiply;
        doc.roots.push(top.id);
    }
    doc.layers.insert(top.id, top);
    doc
}
pub fn render(doc: &Document) -> (Vec<u8>, u64, u64) {
    let root = SharedBudget::new(8 << 20);
    let original = WorkspaceAdmission::new(
        root.reserve_capacity(8 << 20)
            .unwrap()
            .scope(AllocationClass::SharedBudget),
        8 << 20,
        268_435_456,
    );
    original.reserve_work(17).unwrap();
    let callbacks = Cell::new(0_u64);
    let rgba = paintcraft_render::spatial::with_workspace(original.clone(), || {
        paintcraft_render::render_cancellable(doc, || {
            callbacks.set(callbacks.get() + 1);
            Ok(())
        })
    })
    .unwrap();
    let work = original.work_used();
    assert!(work > 17 && work <= 268_435_456);
    assert_eq!(original.scope().footprint().live, 0);
    drop(original);
    assert_eq!(root.usage().reserved, 0);
    (rgba, work, callbacks.get())
}

/// Timing calibration only; never subtracted from completed render latency.
pub fn calibrate() {
    let root = SharedBudget::new(8 << 20);
    let original = WorkspaceAdmission::new(
        root.reserve_capacity(8 << 20)
            .unwrap()
            .scope(AllocationClass::SharedBudget),
        8 << 20,
        268_435_456,
    );
    original.reserve_work(17).unwrap();
    paintcraft_render::spatial::with_workspace(original.clone(), || Ok(())).unwrap();
    std::hint::black_box(original.work_used());
    assert_eq!(original.scope().footprint().live, 0);
    drop(original);
    assert_eq!(root.usage().reserved, 0);
}
