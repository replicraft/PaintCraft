//! PUBLIC straight readback versus premultiplied display bytes are distinct.
//! Expectations are frozen Fraction literals, never CPU product rendering.
use super::*;
use paintcraft_core::types::json::{self, Value};
use paintcraft_core::types::{
    ErrorCode, LayerBlendMode as Mode, Rect, Result,
    resources::{AllocationClass, SharedBudget},
};
use paintcraft_document::{FilterInstance, FilterStack, LayerMask};
use paintcraft_raster::coverage_mask::{A8Mask, Limits};
use paintcraft_render::{
    display::DisplayRequest,
    spatial::{WorkspaceAdmission, with_workspace},
};
fn oracle() -> Value {
    json::parse(include_bytes!("../layer-blend-gpu-fixtures/oracle.json")).unwrap()
}
fn bytes(v: &Value) -> Vec<u8> {
    v.as_array()
        .unwrap()
        .iter()
        .map(|x| u8::try_from(x.as_i64().unwrap()).unwrap())
        .collect()
}
fn rgba(v: &Value) -> Rgba {
    Rgba(bytes(v).try_into().unwrap())
}
fn mode(v: &Value) -> Mode {
    match v.as_str().unwrap() {
        "normal" => Mode::Normal,
        "multiply" => Mode::Multiply,
        "screen" => Mode::Screen,
        _ => panic!("unknown independent fixture mode"),
    }
}
fn gpu() -> Option<GpuRenderer> {
    let adapter = adapter()?;
    let (device, queue) = pollster::block_on(adapter.request_device(&Default::default())).unwrap();
    Some(
        GpuRenderer::new(device, queue)
            .unwrap()
            .with_single_layer_display(true),
    )
}
fn admitted<T>(f: impl FnOnce() -> Result<T>) -> T {
    let b = SharedBudget::new(4 << 20);
    with_workspace(
        WorkspaceAdmission::new(b.scope(AllocationClass::SharedBudget), 2 << 20, 50_000_000),
        f,
    )
    .unwrap()
}
fn mask(rect: Rect, alpha: u8) -> LayerMask {
    LayerMask::new(
        true,
        A8Mask::uniform(
            rect,
            alpha,
            &SharedBudget::new(8192),
            Limits::default(),
            &|| Ok(()),
        )
        .unwrap(),
    )
}
fn scene(width: u32, back: Rgba, front: &[u8], blend: Mode) -> Document {
    let mut d = new_document(width, 1, Some(back), 0, POLICY).unwrap();
    let mut top = paintcraft_document::import_image(width, 1, front, "GPU blend front").unwrap();
    let mut l = top.layers.remove(&top.roots[0]).unwrap();
    l.blend_mode = blend;
    d.roots.push(l.id);
    d.layers.insert(l.id, l);
    d
}
fn top(d: &mut Document) -> &mut Layer {
    let id = *d.roots.last().unwrap();
    d.layers.get_mut(&id).unwrap()
}
#[path = "blends/admission.rs"]
mod admission;
#[path = "blends/numeric.rs"]
mod numeric;
