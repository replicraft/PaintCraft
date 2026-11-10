use super::*;
use paintcraft_core::types::json::{self, Value};

fn oracle() -> Value {
    json::parse(include_bytes!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/tests/display-native-row-oracle.json"
    )))
    .unwrap()
}
fn unhex(value: &Value) -> Vec<u8> {
    value
        .as_str()
        .unwrap()
        .as_bytes()
        .as_chunks::<2>()
        .0
        .iter()
        .map(|p| u8::from_str_radix(std::str::from_utf8(p).unwrap(), 16).unwrap())
        .collect()
}
fn assert_cells(cells: &[DisplayAccumulator], row: &Value) {
    let counts = row.get("counts").unwrap().as_array().unwrap();
    let expected = unhex(row.get("rgba_hex").unwrap());
    assert_eq!(cells.len(), counts.len());
    for (i, cell) in cells.iter().enumerate() {
        assert_eq!(cell.samples(), counts[i].as_i64().unwrap() as u64);
        assert_eq!(
            cell.resolve().unwrap().0.as_slice(),
            &expected[i * 4..i * 4 + 4]
        );
    }
}
fn exact_equal(a: &[DisplayAccumulator], b: &[DisplayAccumulator]) {
    assert_eq!(a.len(), b.len());
    for (a, b) in a.iter().zip(b) {
        assert_eq!(a.samples, b.samples);
        assert_eq!(a.sum.0, b.sum.0);
    }
}

#[test]
fn independent_grouped_rgba_partial_cells_and_offsets() {
    let oracle = oracle();
    for row in oracle.get("cases").unwrap().as_array().unwrap() {
        let pixels = unhex(row.get("pixels_hex").unwrap());
        let step = row.get("step").unwrap().as_i64().unwrap() as u32;
        let offset = row.get("offset").unwrap().as_i64().unwrap() as u32;
        let mut cells = vec![
            DisplayAccumulator::default();
            row.get("counts").unwrap().as_array().unwrap().len()
        ];
        DisplayAccumulator::add_native_row(&mut cells, &pixels, step, offset).unwrap();
        assert_cells(&cells, row);
        // Independent scalar API is a second route, not the source of goldens.
        let mut scalar = vec![DisplayAccumulator::default(); cells.len()];
        for (i, p) in pixels.as_chunks::<4>().0.iter().enumerate() {
            scalar[(offset as usize + i) / step as usize]
                .add(Rgba(*p))
                .unwrap();
        }
        exact_equal(&cells, &scalar);
    }
}

#[test]
fn canonical64_boundary_keeps_global_cell_identity_between_calls() {
    let oracle = oracle();
    let row = oracle.get("sequence").unwrap();
    let mut cells = vec![DisplayAccumulator::default(); 17];
    for call in row.get("calls").unwrap().as_array().unwrap() {
        let start = call.get("start").unwrap().as_i64().unwrap() as usize;
        let count = call.get("count").unwrap().as_i64().unwrap() as usize;
        let pixels = unhex(call.get("pixels_hex").unwrap());
        DisplayAccumulator::add_native_row(
            &mut cells[start..start + count],
            &pixels,
            call.get("step").unwrap().as_i64().unwrap() as u32,
            call.get("offset").unwrap().as_i64().unwrap() as u32,
        )
        .unwrap();
    }
    assert_cells(&cells, row);
}

#[test]
fn malformed_geometry_never_changes_any_destination() {
    for (pixels, step, offset, count) in [
        (vec![], 1, 0, 1),
        (vec![0; 3], 1, 0, 1),
        (vec![0; 260], 1, 0, 65),
        (vec![0; 4], 0, 0, 1),
        (vec![0; 4], 3, 0, 1),
        (vec![0; 4], 1 << 21, 0, 1),
        (vec![0; 4], 4, 4, 1),
        (vec![0; 8], 1, 0, 1),
        (vec![0; 4], 1, 0, 2),
    ] {
        let mut cells = vec![DisplayAccumulator::default(); count];
        for cell in &mut cells {
            cell.add(Rgba([7, 11, 13, 17])).unwrap();
        }
        let before = cells.clone();
        assert_eq!(
            DisplayAccumulator::add_native_row(&mut cells, &pixels, step, offset)
                .unwrap_err()
                .code,
            ErrorCode::InputSchema
        );
        exact_equal(&cells, &before);
    }
}

#[test]
fn overflow_in_later_cell_rolls_back_the_entire_row() {
    let mut cells = [
        DisplayAccumulator {
            sum: Sum([0; 4]),
            samples: RENDER_PIXELS_MAX - 1,
        },
        DisplayAccumulator {
            sum: Sum([0; 4]),
            samples: RENDER_PIXELS_MAX,
        },
    ];
    let before = cells;
    assert_eq!(
        DisplayAccumulator::add_native_row(&mut cells, &[255; 8], 1, 0)
            .unwrap_err()
            .code,
        ErrorCode::ResourceLimit
    );
    exact_equal(&cells, &before);
}

#[test]
fn final_valid_sample_preserves_exact_sum_upper_bound_then_refuses() {
    let n = RENDER_PIXELS_MAX - 1;
    let mut cells = [DisplayAccumulator {
        sum: Sum([65025 * n, 65025 * n, 65025 * n, 255 * n]),
        samples: n,
    }];
    DisplayAccumulator::add_native_row(&mut cells, &[255; 4], 1, 0).unwrap();
    assert_eq!(cells[0].samples(), RENDER_PIXELS_MAX);
    assert_eq!(
        cells[0].sum.0,
        [
            65025 * RENDER_PIXELS_MAX,
            65025 * RENDER_PIXELS_MAX,
            65025 * RENDER_PIXELS_MAX,
            255 * RENDER_PIXELS_MAX
        ]
    );
    assert_eq!(cells[0].resolve().unwrap(), Rgba([255; 4]));
    let before = cells;
    assert_eq!(
        DisplayAccumulator::add_native_row(&mut cells, &[0; 4], 1, 0)
            .unwrap_err()
            .code,
        ErrorCode::ResourceLimit
    );
    exact_equal(&cells, &before);
}

#[test]
fn defining_inline_types_fit_separate512_shadow() {
    assert!(layout_supported());
    assert_eq!(size_of::<DisplayAccumulator>(), 40);
    assert!(size_of::<Result<()>>() <= 112);
    let expected = if usize::BITS == 64 { 464 } else { 304 };
    assert_eq!(inline_control_bytes(), expected);
    assert!(inline_control_bytes() <= 512);
}
