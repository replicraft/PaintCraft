use super::*;
use std::{
    cell::Cell,
    io::{Cursor, Read},
    sync::atomic::{AtomicBool, Ordering},
};
fn read(source: &mut Cursor<Vec<u8>>, out: &mut [u8]) -> Result<usize, String> {
    source.read(out).map_err(|e| e.to_string())
}
#[test]
fn upfront_capacity_precedes_open_and_payload_clones_retain_only_actual_storage() {
    let budget = SharedBudget::new(1024);
    let data = b"\x89PNG\r\n\x1a\nstream".to_vec();
    let expected = data.clone();
    let image = EncodedImage::capture_stream(
        1024,
        &budget,
        &|| Ok(()),
        || {
            assert_eq!(budget.usage().reserved, 1024);
            Ok((Encoding::Png, Cursor::new(data)))
        },
        read,
    )
    .unwrap();
    assert_eq!(image.bytes(), expected);
    assert!(budget.usage().reserved < 1024);
    let clone = image.clone();
    drop(image);
    assert_eq!(clone.bytes(), expected);
    assert!(budget.usage().reserved > 0);
    drop(clone);
    assert_eq!(budget.usage().reserved, 0);
    let opened = Cell::new(false);
    assert!(
        EncodedImage::capture_stream(
            1024,
            &SharedBudget::new(1),
            &|| Ok(()),
            || {
                opened.set(true);
                Ok((Encoding::Png, Cursor::new(vec![])))
            },
            read
        )
        .is_err()
    );
    assert!(!opened.get());
}
#[test]
fn exact_limit_requires_eof_and_excessive_empty_spoofed_or_lying_streams_fail() {
    let data = b"\x89PNG\r\n\x1a\n".to_vec();
    let budget = SharedBudget::new(64);
    assert!(
        EncodedImage::capture_stream(
            8,
            &budget,
            &|| Ok(()),
            || Ok((Encoding::Png, Cursor::new(data.clone()))),
            read
        )
        .is_ok()
    );
    for bytes in [
        vec![],
        b"not PNG".to_vec(),
        [data.clone(), vec![0]].concat(),
    ] {
        assert!(
            EncodedImage::capture_stream(
                8,
                &budget,
                &|| Ok(()),
                || Ok((Encoding::Png, Cursor::new(bytes))),
                read
            )
            .is_err()
        );
        assert_eq!(budget.usage().reserved, 0);
    }
    assert!(
        EncodedImage::capture_stream(
            8,
            &budget,
            &|| Ok(()),
            || Ok((Encoding::Png, ())),
            |_, out| Ok(out.len() + 1)
        )
        .is_err()
    );
    assert_eq!(budget.usage().reserved, 0);
}
#[test]
fn fragmented_transfer_errors_and_every_observed_cancellation_discard_the_snapshot() {
    let budget = SharedBudget::new(128);
    let data = b"\x89PNG\r\n\x1a\nstream".to_vec();
    let calls = Cell::new(0);
    EncodedImage::capture_stream(
        128,
        &budget,
        &|| {
            calls.set(calls.get() + 1);
            Ok(())
        },
        || Ok((Encoding::Png, Cursor::new(data.clone()))),
        |s, out| s.read(&mut out[..1]).map_err(|e| e.to_string()),
    )
    .unwrap();
    for failure in 0..calls.get() {
        let count = Cell::new(0);
        let result = EncodedImage::capture_stream(
            128,
            &budget,
            &|| {
                let n = count.get();
                count.set(n + 1);
                if n == failure {
                    Err("cancelled".into())
                } else {
                    Ok(())
                }
            },
            || Ok((Encoding::Png, Cursor::new(data.clone()))),
            |s, out| s.read(&mut out[..1]).map_err(|e| e.to_string()),
        );
        assert!(result.is_err());
        assert_eq!(budget.usage().reserved, 0);
    }
    assert!(
        EncodedImage::capture_stream(
            128,
            &budget,
            &|| Ok(()),
            || Ok((Encoding::Png, ())),
            |_, _| Err("read failed".into())
        )
        .is_err()
    );
    assert_eq!(budget.usage().reserved, 0);
    let dropped = Arc::new(AtomicBool::new(false));
    struct Source(Arc<AtomicBool>);
    impl Drop for Source {
        fn drop(&mut self) {
            self.0.store(true, Ordering::SeqCst);
        }
    }
    assert!(
        EncodedImage::capture_stream(
            128,
            &budget,
            &|| Ok(()),
            || Ok((Encoding::Png, Source(dropped.clone()))),
            |_, _| Err("read failed".into())
        )
        .is_err()
    );
    assert!(dropped.load(Ordering::SeqCst));
    assert_eq!(budget.usage().reserved, 0);
}
