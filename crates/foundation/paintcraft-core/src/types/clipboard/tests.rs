use super::*;
use std::cell::Cell;

#[test]
fn exact_pixels_clone_and_capacity_ownership() {
    let budget = SharedBudget::new(64);
    let mut bytes = Vec::with_capacity(32);
    bytes.extend_from_slice(&[12, 44, 88, 0, 255, 2, 1, 1]);
    let capacity = bytes.capacity() as u64;
    let image = Image::from_rgba(2, 1, bytes, &budget, &|| Ok(())).unwrap();
    let clone = image.clone();
    assert_eq!(image.rgba(), &[12, 44, 88, 0, 255, 2, 1, 1]);
    assert_eq!((image.width(), image.height()), (2, 1));
    assert_eq!(image.rgba().as_ptr(), clone.rgba().as_ptr());
    assert_eq!(budget.usage().reserved, capacity);
    drop(image);
    assert_eq!(budget.usage().reserved, capacity);
    drop(clone);
    assert_eq!(budget.usage().reserved, 0);
}

#[test]
fn invalid_dimensions_lengths_and_quota_do_not_read() {
    let budget = SharedBudget::new(7);
    let read = |_, _| -> Result<Rgba> { panic!("denied capture read source") };
    for (w, h) in [(0, 1), (1, 0), (u32::MAX, u32::MAX), (2, 1)] {
        assert!(Image::from_fn(w, h, &budget, &|| Ok(()), read).is_err());
        assert_eq!(budget.usage().reserved, 0);
    }
    assert_eq!(
        Image::from_rgba(1, 1, vec![0; 3], &budget, &|| Ok(()))
            .unwrap_err()
            .code,
        ErrorCode::InputSchema
    );
    let mut capacity = Vec::with_capacity(8);
    capacity.extend_from_slice(&[0; 4]);
    assert!(Image::from_rgba(1, 1, capacity, &budget, &|| Ok(())).is_err());
    assert_eq!(budget.usage().reserved, 0);
}

#[test]
fn cancellation_at_every_checkpoint_releases_entire_private_attempt() {
    let budget = SharedBudget::new(4096);
    let calls = Cell::new(0);
    let check = || {
        calls.set(calls.get() + 1);
        Ok(())
    };
    let image = Image::from_fn(130, 3, &budget, &check, |x, y| {
        Ok(Rgba([x as u8, y as u8, 7, 0]))
    })
    .unwrap();
    let checkpoints = calls.get();
    assert_eq!(&image.rgba()[4 * 131..4 * 132], &[1, 1, 7, 0]);
    drop(image);
    for cancel_at in 1..=checkpoints {
        calls.set(0);
        let check = || {
            calls.set(calls.get() + 1);
            if calls.get() == cancel_at {
                Err(Error::new(ErrorCode::OperationCancelled, "test"))
            } else {
                Ok(())
            }
        };
        assert_eq!(
            Image::from_fn(130, 3, &budget, &check, |_, _| Ok(Rgba::TRANSPARENT))
                .unwrap_err()
                .code,
            ErrorCode::OperationCancelled
        );
        assert_eq!(budget.usage().reserved, 0);
    }
    assert!(
        Image::from_fn(3, 3, &budget, &|| Ok(()), |x, y| {
            if x == 1 && y == 2 {
                Err(Error::new(ErrorCode::StorageFailed, "source"))
            } else {
                Ok(Rgba::TRANSPARENT)
            }
        })
        .is_err()
    );
    assert_eq!(budget.usage().reserved, 0);
}

#[test]
fn admitted_writer_is_zeroed_private_and_releases_partial_failures() {
    let budget = SharedBudget::new(8);
    let image = Image::from_writer(2, 1, &budget, &|| Ok(()), |pixels| {
        assert_eq!(pixels, &[0; 8]);
        pixels.copy_from_slice(&[1, 2, 3, 0, 4, 5, 6, 127]);
        Ok(())
    })
    .unwrap();
    let clone = image.clone();
    assert_eq!(image.rgba(), &[1, 2, 3, 0, 4, 5, 6, 127]);
    assert_eq!(budget.usage().reserved, 8);
    assert!(
        Image::from_writer(1, 1, &budget, &|| Ok(()), |_| panic!(
            "denied writer called"
        ))
        .is_err()
    );
    drop(image);
    assert_eq!(budget.usage().reserved, 8);
    drop(clone);
    let failure = Image::from_writer(2, 1, &budget, &|| Ok(()), |pixels| {
        pixels[0] = 99;
        Err(Error::new(ErrorCode::StorageFailed, "test"))
    })
    .unwrap_err();
    assert_eq!(failure.code, ErrorCode::StorageFailed);
    assert_eq!(budget.usage().reserved, 0);
}

#[test]
fn decoder_output_transfers_pre_admitted_capacity_and_retains_exact_pixels() {
    let budget = SharedBudget::new(64);
    let reservation = budget.reserve_capacity(64).unwrap();
    let image = Image::from_rgba_reserved(
        2,
        1,
        vec![17, 44, 99, 0, 1, 2, 3, 127],
        &reservation,
        &|| Ok(()),
    )
    .unwrap();
    assert_eq!(budget.usage().reserved, 64);
    assert_eq!(image.rgba(), &[17, 44, 99, 0, 1, 2, 3, 127]);
    drop(reservation);
    assert_eq!(budget.usage().reserved, 8);
    let clone = image.clone();
    drop(image);
    assert_eq!(budget.usage().reserved, 8);
    drop(clone);
    assert_eq!(budget.usage().reserved, 0);
}
#[test]
fn reserved_decode_failures_release_their_owned_pixels_and_transfers() {
    let budget = SharedBudget::new(8);
    let reservation = budget.reserve_capacity(8).unwrap();
    assert!(Image::from_rgba_reserved(3, 1, vec![0; 12], &reservation, &|| Ok(())).is_err());
    assert_eq!(reservation.available(), 8);
    assert!(Image::from_rgba_reserved(1, 1, vec![0; 3], &reservation, &|| Ok(())).is_err());
    assert_eq!(reservation.available(), 8);
    let calls = Cell::new(0);
    let check = || {
        calls.set(calls.get() + 1);
        if calls.get() == 2 {
            Err(Error::new(ErrorCode::OperationCancelled, "test"))
        } else {
            Ok(())
        }
    };
    assert!(Image::from_rgba_reserved(1, 1, vec![0; 4], &reservation, &check).is_err());
    assert_eq!(reservation.available(), 8);
    drop(reservation);
    assert_eq!(budget.usage().reserved, 0);
}
