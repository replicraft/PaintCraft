use super::*;
use std::sync::atomic::{AtomicUsize, Ordering};
#[test]
fn cancellable_import_preserves_ids_hidden_rgb_and_partial_edge_tiles() {
    let (width, height) = (257, 2);
    let mut rgba = vec![0; width as usize * height as usize * 4];
    for (index, pixel) in rgba.as_chunks_mut::<4>().0.iter_mut().enumerate() {
        *pixel = [index as u8, (index >> 8) as u8, 97, (index % 256) as u8];
    }
    let expected = import_image(width, height, &rgba, "independent-source").unwrap();
    let actual =
        import_image_checked(width, height, &rgba, "independent-source", &|| Ok(())).unwrap();
    assert!(actual.same_semantics(&expected));
    assert_eq!(actual.state_id, expected.state_id);
    let raster = &actual.layers_in_order().next().unwrap().raster;
    for y in 0..height {
        for x in 0..width {
            let index = (y * width + x) as usize;
            assert_eq!(
                raster.get(x.into(), y.into()).0,
                rgba[index * 4..index * 4 + 4]
            );
        }
    }
}
#[test]
fn cancellation_during_ingestion_cannot_return_partial_document() {
    let checks = AtomicUsize::new(0);
    let rgba = vec![255; 257 * 2 * 4];
    let error = import_image_checked(257, 2, &rgba, "source", &|| {
        if checks.fetch_add(1, Ordering::Relaxed) >= 4 {
            Err(Error::new(
                ErrorCode::OperationCancelled,
                "cancel document assembly",
            ))
        } else {
            Ok(())
        }
    })
    .unwrap_err();
    assert_eq!(error.code, ErrorCode::OperationCancelled);
}
