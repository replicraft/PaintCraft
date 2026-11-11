//! Finite fixtures and independently fixed PNG samples, shared by actual CLI/stdio tests.
use paintcraft_core::types::{
    LayerBlendMode, Rect, Rgba, filters::Descriptor, resources::SharedBudget,
};
use paintcraft_document::{
    Document, LayerMask, POLICY,
    filters::{FilterInstance, FilterMask, FilterStack},
    new_document,
};
use paintcraft_raster::coverage_mask::{A8Mask, Band, Limits, Run};
use serde_json::{Value, json};
use std::{
    fs,
    path::PathBuf,
    process::{Command, Output},
};
pub const STYLE_SCHEMA: &str = "paintcraft.batch/layer-styles-v1";
pub const EXPECTED_COMPOUND: [u8; 8] = [50, 108, 189, 144, 49, 109, 192, 160];
pub struct Folder(pub PathBuf);
impl Folder {
    pub fn new() -> Self {
        let p = std::env::temp_dir().join(format!("paintcraft-styles-{}", uuid::Uuid::new_v4()));
        fs::create_dir(&p).unwrap();
        Self(p)
    }
    pub fn path(&self, name: &str) -> PathBuf {
        self.0.join(name)
    }
}
impl Drop for Folder {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}
pub fn run(args: &[&str]) -> Output {
    Command::new(env!("CARGO_BIN_EXE_paintcraft-cli"))
        .args(args)
        .output()
        .unwrap()
}
pub fn success(output: Output) -> Value {
    assert!(
        output.status.success(),
        "{} {}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    serde_json::from_slice(&output.stdout).unwrap()
}
pub fn descriptor(rgb: [u8; 3], opacity: u8) -> Value {
    json!({"kind":"color-overlay-rgb-v1","color":rgb,"opacity_u8":opacity,"effect_blend_mode":"normal"})
}
pub fn base() -> Document {
    new_document(2, 1, Some(Rgba([10, 20, 30, 255])), 0, POLICY).unwrap()
}
pub fn packet(doc: &Document, operations: Value, features: Value) -> Value {
    json!({"schema":STYLE_SCHEMA,"atomic":true,"policy":POLICY,"base":{"state_id":doc.state_id.to_string(),"publication_epoch":"0"},"required_filters":[],"required_layer_features":features,"operations":operations})
}
pub fn add(layer: uuid::Uuid, rgb: [u8; 3], opacity: u8) -> Value {
    json!({"op":"layer.style.add","layer_ref":{"id":layer.to_string()},"ref":"overlay","descriptor":descriptor(rgb,opacity)})
}
pub fn png(bytes: &[u8]) -> Vec<u8> {
    let mut decoder = png::Decoder::new(std::io::Cursor::new(bytes));
    decoder.set_transformations(png::Transformations::EXPAND | png::Transformations::STRIP_16);
    let mut reader = decoder.read_info().unwrap();
    let mut rgba = vec![0; reader.output_buffer_size().unwrap()];
    let info = reader.next_frame(&mut rgba).unwrap();
    assert_eq!((info.width, info.height), (2, 1));
    assert_eq!(info.color_type, png::ColorType::Rgba);
    rgba.truncate(info.buffer_size());
    rgba
}
fn mask(left: u8, right: u8, budget: &SharedBudget) -> A8Mask {
    let runs = [
        Run {
            x0: 0,
            x1: 1,
            coverage: left,
        },
        Run {
            x0: 1,
            x1: 2,
            coverage: right,
        },
    ];
    A8Mask::from_bands(
        Rect::new(0, 0, 2, 1).unwrap(),
        &[Band {
            y0: 0,
            y1: 1,
            runs: &runs,
        }],
        budget,
        Limits::default(),
        &|| Ok(()),
    )
    .unwrap()
}
/// Unstyled input; only the actual batch adds the Color Overlay.
pub fn compound() -> Document {
    let mut doc = new_document(2, 1, Some(Rgba([31, 127, 241, 128])), 0, POLICY).unwrap();
    let mut other = new_document(2, 1, Some(Rgba([204, 77, 13, 128])), 1, POLICY).unwrap();
    let id = other.roots[0];
    let mut top = other.layers.remove(&id).unwrap();
    let budget = SharedBudget::new(1 << 20);
    let filter = FilterInstance::new(uuid::Uuid::from_u128(14), true, Descriptor::InvertRgbV1)
        .with_mask(Some(FilterMask::new(true, mask(0, 255, &budget))));
    top.filters = FilterStack::new(vec![filter]).unwrap();
    top.mask = Some(LayerMask::new(true, mask(127, 255, &budget)));
    top.opacity = 128;
    top.blend_mode = LayerBlendMode::Multiply;
    assert_ne!(id, doc.roots[0]);
    doc.roots.push(id);
    doc.layers.insert(id, top);
    doc
}
