use paintcraft_core::types::{Error, ErrorCode, Rect, Rgba};
use paintcraft_document::{POLICY, new_document};
use paintcraft_raster::{Admission, BlendMode};
use paintcraft_render::{render_flat, render_region, render_region_cancellable};

#[test]
fn tiny_regions_of_enormous_sparse_documents_do_not_materialize_the_canvas() {
    let doc = new_document(1_048_576, 1_048_576, Some(Rgba([1, 2, 3, 255])), 1, POLICY).unwrap();
    let region = Rect::new(900_000, 850_000, 3, 2).unwrap();
    assert_eq!(
        render_region(&doc, region).unwrap(),
        [1, 2, 3, 255].repeat(6)
    );
    assert_eq!(
        render_flat(&doc).unwrap_err().code,
        ErrorCode::ResourceLimit
    );
}

#[test]
fn arbitrary_output_partitions_equal_the_complete_clipped_ordered_image() {
    let mut doc = new_document(259, 257, Some(Rgba([17, 45, 82, 255])), 4, POLICY).unwrap();
    let mut top = doc.layers[&doc.roots[0]].clone();
    top.id = paintcraft_core::types::ids::deterministic_id(&["regional-top"]);
    top.origin = (-5, 3);
    top.opacity = 128;
    top.raster
        .blend_pixel(
            255,
            253,
            Rgba([255, 0, 0, 128]),
            BlendMode::Replace,
            &mut Admission::new(65536),
        )
        .unwrap();
    doc.roots.push(top.id);
    doc.layers.insert(top.id, top);
    let full = render_flat(&doc).unwrap();
    for edge in [1, 2, 3, 64, 128, 256] {
        let mut assembled = vec![0; full.len()];
        for y in (0..257).step_by(edge) {
            for x in (0..259).step_by(edge) {
                let rect = Rect::new(
                    x as i64,
                    y as i64,
                    edge.min(259 - x) as i64,
                    edge.min(257 - y) as i64,
                )
                .unwrap();
                let part = render_region(&doc, rect).unwrap();
                let stride = rect.width() as usize * 4;
                for (row, bytes) in part.chunks_exact(stride).enumerate() {
                    let offset = ((y + row) * 259 + x) * 4;
                    assembled[offset..offset + stride].copy_from_slice(bytes);
                }
            }
        }
        assert_eq!(assembled, full, "edge {edge}");
    }
    // Hand-calculated alpha-64 red over the unchanged opaque base pixel.
    assert_eq!(
        render_region(&doc, Rect::new(250, 256, 1, 1).unwrap()).unwrap(),
        [77, 34, 61, 255]
    );
}

#[test]
fn malformed_ranges_and_cancellation_are_rejected_without_partial_output() {
    let doc = new_document(20, 20, None, 0, POLICY).unwrap();
    for rect in [
        Rect {
            x0: i64::MIN,
            y0: 0,
            x1: i64::MAX,
            y1: 1,
        },
        Rect {
            x0: 0,
            y0: 0,
            x1: 0,
            y1: 1,
        },
        Rect::new(19, 19, 2, 2).unwrap(),
    ] {
        assert_eq!(
            render_region(&doc, rect).unwrap_err().code,
            ErrorCode::InputSchema
        );
    }
    assert_eq!(
        render_region_cancellable(&doc, doc.canvas_rect(), || Err(Error::new(
            ErrorCode::OperationCancelled,
            "cancel"
        )))
        .unwrap_err()
        .code,
        ErrorCode::OperationCancelled
    );
}
