pub mod reference;
use paintcraft_core::types::{Rect, Rgba};
use paintcraft_document::{Document, Layer, POLICY, new_document};
use paintcraft_raster::{Admission, BlendMode, RasterLayer};

pub const CASES: &[&str] = &[
    "tiny",
    "opaque1",
    "sparse1",
    "sparse5",
    "sparse20",
    "sparse100",
    "dense5",
    "dense-opaque5",
    "dense-opaque-odd5",
    "short-opaque5",
    "uniform5",
    "clipped100",
];

/// Fixed seed and geometry; fixtures are generated once outside steady-state timing.
pub fn fixture(case: &str) -> Document {
    assert!(CASES.contains(&case));
    let side = match case {
        "tiny" => 16,
        "short-opaque5" => 17,
        "dense-opaque-odd5" => 1025,
        _ => 1024,
    };
    let opaque = case.contains("opaque") && case != "opaque1";
    let dense = case == "dense5" || opaque;
    let count = match case {
        "sparse5" | "dense5" | "uniform5" | "dense-opaque5" | "dense-opaque-odd5"
        | "short-opaque5" => 5,
        "sparse20" => 20,
        "sparse100" | "clipped100" => 100,
        _ => 1,
    };
    let mut doc = new_document(side, side, Some(Rgba([255; 4])), 42, POLICY).unwrap();
    doc.layers.clear();
    doc.roots.clear();
    let mut adm = Admission::new(1 << 28);
    for n in 0..count {
        let id = paintcraft_core::types::ids::deterministic_id(&["bench", &n.to_string()]);
        let default = if case == "uniform5" {
            Rgba([31 + n as u8 * 17, 125, 204, 87 + n as u8 * 23])
        } else if case == "opaque1" {
            Rgba([21, 65, 170, 255])
        } else {
            Rgba([45, 89, 123, 0]) // Deliberately nonzero hidden RGB.
        };
        let mut raster = RasterLayer::new(side, side, default);
        if dense {
            let pixels: Vec<u8> = (0..side * side)
                .flat_map(|p| {
                    [
                        p as u8,
                        (p / side) as u8,
                        (p.wrapping_mul(17) + n) as u8,
                        if opaque && n == 0 {
                            255
                        } else {
                            [0, 1, 64, 128, 254, 255][p as usize % 6]
                        },
                    ]
                })
                .collect();
            raster = RasterLayer::from_rgba(side, side, default, &pixels, &mut adm).unwrap();
        }
        if case != "opaque1" && !opaque {
            let x = i64::from((n * 31) % (side - 7));
            let y = i64::from((n * 47) % (side - 7));
            raster
                .fill_rect(
                    &Rect::from_origin_size(x, y, 7, 7).unwrap(),
                    Rgba([210, n as u8, 33, 173]),
                    BlendMode::Replace,
                    &mut adm,
                )
                .unwrap();
        }
        let layer = Layer {
            id,
            name: format!("Fixture {n}"),
            origin: if case == "clipped100" {
                (2048, -2048)
            } else {
                (0, 0)
            },
            visible: true,
            opacity: if dense && !(opaque && n == 0) {
                173
            } else {
                255
            },
            filters: Default::default(),
            raster,
        };
        doc.roots.push(id);
        doc.layers.insert(id, layer);
    }
    doc
}

pub fn verify(doc: &Document) {
    assert_eq!(
        reference::reference(doc, || Ok(())).unwrap(),
        paintcraft_render::render_flat(doc).unwrap()
    );
}
