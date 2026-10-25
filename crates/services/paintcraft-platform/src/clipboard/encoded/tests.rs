use super::*;
use std::cell::Cell;
fn png() -> Vec<u8> {
    b"\x89PNG\r\n\x1a\nencoded transport, not decoded pixels".to_vec()
}
#[test]
fn admission_precedes_read_and_clones_retain_exact_capacity_until_last_reader() {
    let budget = SharedBudget::new(256);
    let data = png();
    let length = data.len();
    let mut owned = Vec::with_capacity(128);
    owned.extend_from_slice(&data);
    let image = EncodedImage::capture(Encoding::Png, length, &budget, &|| Ok(()), || {
        assert_eq!(budget.usage().reserved, length as u64);
        Ok(owned)
    })
    .unwrap();
    assert_eq!(image.bytes(), data);
    let diagnostic = format!("{image:?}");
    assert!(diagnostic.contains("Png"));
    assert!(!diagnostic.contains("encoded transport"));
    assert!(!diagnostic.contains("137, 80"));

    assert_eq!(image.encoding(), Encoding::Png);
    assert_eq!(image.encoding().mime(), "image/png");
    assert_eq!(budget.usage().reserved, 128);
    let clone = image.clone();
    assert_eq!(clone.bytes().as_ptr(), image.bytes().as_ptr());
    drop(image);
    assert_eq!(budget.usage().reserved, 128);
    drop(clone);
    assert_eq!(budget.usage().reserved, 0);
}
#[test]
fn oversized_lengths_quota_and_extra_capacity_denials_leave_no_capture() {
    for length in [0, MAX_ENCODED_BYTES as usize + 1, 16] {
        let budget = SharedBudget::new(0);
        assert!(
            EncodedImage::capture(Encoding::Png, length, &budget, &|| Ok(()), || panic!(
                "denied source read"
            ))
            .is_err()
        );
        assert_eq!(budget.usage().reserved, 0);
    }
    let budget = SharedBudget::new(64);
    let mut data = Vec::with_capacity(128);
    data.extend_from_slice(&png());
    assert!(
        EncodedImage::capture(Encoding::Png, data.len(), &budget, &|| Ok(()), || Ok(data)).is_err()
    );
    assert_eq!(budget.usage().reserved, 0);
}
#[test]
fn changing_lengths_spoofed_types_and_source_failure_release_all_admission() {
    let budget = SharedBudget::new(256);
    assert!(
        EncodedImage::capture(Encoding::Png, 12, &budget, &|| Ok(()), || Ok(png()))
            .unwrap_err()
            .contains("changed")
    );
    for encoding in [Encoding::Png, Encoding::Tiff] {
        assert!(
            EncodedImage::capture(encoding, 8, &budget, &|| Ok(()), || Ok(vec![0; 8]))
                .unwrap_err()
                .contains("signature")
        );
        assert_eq!(budget.usage().reserved, 0);
    }
    assert!(
        EncodedImage::capture(Encoding::Png, 12, &budget, &|| Ok(()), || Err(
            "source failed".into()
        ))
        .is_err()
    );
    assert_eq!(budget.usage().reserved, 0);
    for bytes in [b"II*\0payload".as_slice(), b"MM\0*payload".as_slice()] {
        let image = EncodedImage::capture(Encoding::Tiff, bytes.len(), &budget, &|| Ok(()), || {
            Ok(bytes.to_vec())
        })
        .unwrap();
        assert_eq!(image.bytes(), bytes);
    }
    assert_eq!(budget.usage().reserved, 0);
}
#[test]
fn every_observed_cancellation_releases_the_private_attempt() {
    let budget = SharedBudget::new(256);
    let data = png();
    let calls = Cell::new(0);
    let count = || {
        calls.set(calls.get() + 1);
        Ok(())
    };
    drop(
        EncodedImage::capture(Encoding::Png, data.len(), &budget, &count, || {
            Ok(data.clone())
        })
        .unwrap(),
    );
    for stop in 1..=calls.get() {
        let current = Cell::new(0);
        let check = || {
            current.set(current.get() + 1);
            if current.get() == stop {
                Err("cancelled".into())
            } else {
                Ok(())
            }
        };
        assert!(
            EncodedImage::capture(Encoding::Png, data.len(), &budget, &check, || Ok(
                data.clone()
            ))
            .unwrap_err()
            .contains("cancelled")
        );
        assert_eq!(budget.usage().reserved, 0);
    }
}
