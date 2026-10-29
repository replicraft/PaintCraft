use super::*;
use std::cell::Cell;
#[test]
fn actual_rgba_thumbnail_preserves_small_samples_and_skinny_axes_without_upscaling() {
    let rgba = std::fs::read(
        std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../paintcraft-cli/tests/jxr-export-fixtures/4x3.rgba"),
    )
    .unwrap();
    let small = thumbnail(4, 3, &rgba, &|| Ok(())).unwrap();
    assert_eq!(small.size, [4, 3]);
    assert_eq!(
        small
            .pixels
            .iter()
            .flat_map(|p| p.to_array())
            .collect::<Vec<_>>(),
        rgba
    );
    let mut wide = vec![255; 65536 * 4];
    for (x, p) in wide.as_chunks_mut::<4>().0.iter_mut().enumerate() {
        p[..3].copy_from_slice(&[(x % 251) as u8, 31, 219]);
    }
    let image = thumbnail(65536, 1, &wide, &|| Ok(())).unwrap();
    assert_eq!(image.size, [192, 1]);
    assert_eq!(image.pixels[0].to_array(), [0, 31, 219, 255]);
    assert_eq!(
        image.pixels[191].to_array(),
        [(65194usize % 251) as u8, 31, 219, 255]
    );
    let tall = thumbnail(1, 65536, &wide, &|| Ok(())).unwrap();
    assert_eq!(tall.size, [1, 192]);
    assert_eq!(tall.pixels[191], image.pixels[191]);
}
#[test]
fn thumbnail_cancellation_never_returns_a_partial_visible_image() {
    let rgba = vec![255; 256 * 256 * 4];
    let calls = Cell::new(0);
    thumbnail(256, 256, &rgba, &|| {
        calls.set(calls.get() + 1);
        Ok(())
    })
    .unwrap();
    for stop in [0, 1, calls.get() - 1] {
        let n = Cell::new(0);
        let error = thumbnail(256, 256, &rgba, &|| {
            let at = n.get();
            n.set(at + 1);
            if at == stop {
                Err(Error::new(
                    ErrorCode::OperationCancelled,
                    "cancel thumbnail",
                ))
            } else {
                Ok(())
            }
        })
        .unwrap_err();
        assert_eq!(error.code, ErrorCode::OperationCancelled);
    }
}
