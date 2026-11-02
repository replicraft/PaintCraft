//! Shared complete production-region fixtures; no substitute numerical kernel.
use paintcraft_core::types::{
    LayerBlendMode as Mode, Rect, Rgba,
    ids::deterministic_id,
    resources::{AllocationClass, SharedBudget},
};
use paintcraft_document::{Document, POLICY};
use paintcraft_raster::{Admission, BlendMode, RasterLayer};
use paintcraft_render::spatial::{WorkspaceAdmission, with_workspace};
use std::hint::black_box;

pub const BYTES: u64 = 64 << 20;
pub const WORK: u64 = 1_000_000_000;
#[derive(Clone, Copy, Debug)]
pub struct Case {
    pub id: &'static str,
    pub width: u32,
    pub height: u32,
    pub mode: Mode,
    pub expected: [u8; 4],
}
pub const CASES: [Case; 4] = [
    Case {
        id: "default-multiply",
        width: 1000,
        height: 486,
        mode: Mode::Multiply,
        expected: [29, 105, 184, 255],
    },
    Case {
        id: "default-screen",
        width: 1000,
        height: 486,
        mode: Mode::Screen,
        expected: [76, 137, 241, 255],
    },
    Case {
        id: "1080p-multiply",
        width: 1920,
        height: 1080,
        mode: Mode::Multiply,
        expected: [29, 105, 184, 255],
    },
    Case {
        id: "1080p-screen",
        width: 1920,
        height: 1080,
        mode: Mode::Screen,
        expected: [76, 137, 241, 255],
    },
];
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Variant {
    Opaque,
    General,
}
impl Variant {
    pub fn name(self) -> &'static str {
        match self {
            Self::Opaque => "opaque",
            Self::General => "general",
        }
    }
    pub fn parse(name: &str) -> Self {
        match name {
            "opaque" => Self::Opaque,
            "general" => Self::General,
            _ => panic!("unknown variant"),
        }
    }
}
pub fn case(name: &str) -> Case {
    *CASES
        .iter()
        .find(|case| case.id == name)
        .expect("unknown case")
}
#[derive(Debug)]
pub struct Fixture {
    pub document: Document,
    pub roi: Rect,
    pub case: Case,
}
pub fn fixture(case: Case, variant: Variant) -> Fixture {
    let document_height = case.height.div_ceil(256) * 256 + 256;
    let mut doc = paintcraft_document::new_document(
        case.width,
        document_height,
        Some(Rgba([31, 127, 241, 255])),
        42,
        POLICY,
    )
    .unwrap();
    let back = doc.roots[0];
    if variant == Variant::General {
        let raster = &mut doc.layers.get_mut(&back).unwrap().raster;
        let y = i64::from(document_height - 1);
        let mut admission = Admission::new(65_536);
        // Genuine existing paint APIs, strictly outside ROI. Painting back does
        // not prune the tile: preserve that resident ownership deliberately.
        raster
            .blend_pixel(
                0,
                y,
                Rgba([32, 127, 241, 255]),
                BlendMode::Replace,
                &mut admission,
            )
            .unwrap();
        raster
            .blend_pixel(
                0,
                y,
                Rgba([31, 127, 241, 255]),
                BlendMode::Replace,
                &mut admission,
            )
            .unwrap();
        assert_eq!(raster.resident_tiles(), 1);
    } else {
        assert_eq!(doc.layers[&back].raster.resident_tiles(), 0);
    }
    let mut front = doc.layers[&back].clone();
    front.id = deterministic_id(&["blend-benchmark-front"]);
    front.name = "Blend benchmark front".into();
    front.blend_mode = case.mode;
    front.opacity = 128;
    front.raster = RasterLayer::new(case.width, document_height, Rgba([204, 77, 13, 128]));
    doc.roots.push(front.id);
    doc.layers.insert(front.id, front);
    Fixture {
        document: doc,
        roi: Rect::new(0, 0, i64::from(case.width), i64::from(case.height)).unwrap(),
        case,
    }
}
fn workspace() -> WorkspaceAdmission {
    WorkspaceAdmission::new(
        SharedBudget::new(BYTES).scope(AllocationClass::SharedBudget),
        BYTES,
        WORK,
    )
}
pub fn render(fixture: &Fixture) -> Vec<u8> {
    // One explicit production caller per complete frame, never reset within it.
    with_workspace(workspace(), || {
        paintcraft_render::render_region(black_box(&fixture.document), black_box(fixture.roi))
    })
    .unwrap()
}
pub fn calibrate() {
    // Zero-render caller-factory calibration, retained separately, not subtracted.
    with_workspace(workspace(), || {
        black_box(());
        Ok(())
    })
    .unwrap();
}
pub fn verify(case: Case) {
    let mut results = Vec::new();
    for variant in [Variant::Opaque, Variant::General] {
        let fixture = fixture(case, variant);
        let q = paintcraft_render::blend::quote_document(&fixture.document, fixture.roi).unwrap();
        let pixels = u64::from(case.width) * u64::from(case.height);
        assert_eq!(
            (q.opaque_samples, q.general_samples),
            match variant {
                Variant::Opaque => (pixels, 0),
                Variant::General => (0, pixels),
            },
            "genuine public quote branch, not an injected private Kind"
        );
        let admission = workspace();
        let output = with_workspace(admission.clone(), || {
            paintcraft_render::render_region(&fixture.document, fixture.roi)
        })
        .unwrap();
        assert_eq!(output.len(), pixels as usize * 4);
        assert!(
            output
                .as_chunks::<4>()
                .0
                .iter()
                .all(|p| *p == case.expected),
            "every actual output byte must match independent Fraction literal"
        );
        println!(
            "{{\"case\":\"{}\",\"variant\":\"{}\",\"allBytesEqualLiteral\":true,\"outputBytes\":{},\"rgba\":{:?},\"workUsed\":{},\"controlBytes\":{},\"opaqueSamples\":{},\"generalSamples\":{},\"backgroundResidentTiles\":{}}}",
            case.id,
            variant.name(),
            output.len(),
            case.expected,
            admission.work_used(),
            q.control_bytes,
            q.opaque_samples,
            q.general_samples,
            fixture.document.layers[&fixture.document.roots[0]]
                .raster
                .resident_tiles()
        );
        results.push(output);
    }
    assert_eq!(
        results[0], results[1],
        "identical requested ROI visible bytes"
    );
}
pub fn verify_all() {
    for case in CASES {
        verify(case);
    }
}
