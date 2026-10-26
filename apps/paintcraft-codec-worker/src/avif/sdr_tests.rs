//! Actual selected C decoder proof against independent native planes/Fraction RGBA.
use super::execute::decode_item;
use paintcraft_core::types::{Error, ErrorCode};
use paintcraft_formats::avif;
use std::{cell::Cell, path::PathBuf};
fn fixture(name: &str) -> Vec<u8> {
    std::fs::read(
        PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("../../crates/services/paintcraft-formats/tests/avif-sdr-fixtures")
            .join(name),
    )
    .unwrap()
}
#[test]
fn selected_dav1d_planes_and_existing_color_converter_match_independent_sdr_oracles() {
    let mut cases = 0;
    for size in ["even", "odd"] {
        let alpha_planes = fixture(&format!("alpha-{size}.planes"));
        for (layout, siting) in [
            ("444", "fullchroma"),
            ("420", "vertical"),
            ("420", "colocated"),
        ] {
            for range in ["full", "limited"] {
                let stem = format!("{layout}-{siting}-{range}-{size}");
                let expected_planes = fixture(&format!("{stem}.planes"));
                for transparent in [false, true] {
                    let name = format!("{stem}{}", if transparent { "-alpha" } else { "" });
                    let source = fixture(&format!("{name}.avif"));
                    let admitted =
                        avif::sdr::admit_ycbcr8(&source, avif::INSPECT_PIXELS_MAX, &|| Ok(()))
                            .unwrap();
                    let count = admitted.primary.width as usize * admitted.primary.height as usize;
                    let primary = decode_item(&admitted.primary).unwrap();
                    assert_eq!(
                        primary.planes, expected_planes,
                        "{name}: native primary planes"
                    );
                    let alpha = admitted
                        .alpha
                        .as_ref()
                        .map(|item| decode_item(item).unwrap());
                    if let Some(alpha) = &alpha {
                        assert_eq!(
                            alpha.planes, alpha_planes,
                            "{name}: independent alpha planes"
                        );
                    }
                    let rgba = avif::sdr::reconstruct(
                        &admitted,
                        source.len() as u64,
                        &primary.planes,
                        alpha.as_ref().map(|value| &value.planes[..count]),
                        &|| Ok(()),
                    )
                    .unwrap();
                    let reference = fixture(&format!("{name}.rational.rgba"));
                    assert_eq!(rgba.len(), reference.len());
                    let mut maximum = 0;
                    let mut differences = 0;
                    let mut hidden = 0;
                    for (pixel, expected) in rgba
                        .as_chunks::<4>()
                        .0
                        .iter()
                        .zip(reference.as_chunks::<4>().0)
                    {
                        for channel in 0..3 {
                            let difference = pixel[channel].abs_diff(expected[channel]);
                            maximum = maximum.max(difference);
                            differences += usize::from(difference != 0);
                            assert_eq!(
                                pixel[channel], expected[channel],
                                "{name}: exact Fraction RGB"
                            );
                        }
                        assert_eq!(pixel[3], expected[3], "{name}: alpha must be exact");
                        if pixel[3] == 0 {
                            hidden += usize::from(pixel[..3].iter().any(|value| *value != 0));
                        }
                    }
                    if transparent {
                        assert!(hidden > 0, "{name}: meaningful hidden RGB");
                    }
                    println!(
                        "{name}: max RGB difference={maximum}; differing channels={differences}; exact alpha; nonzero hidden RGB pixels={hidden}"
                    );
                    cases += 1;
                }
            }
        }
    }
    assert_eq!(cases, 24);
}
#[test]
fn reconstruction_refuses_changed_planes_and_cancel_without_partial_pixels() {
    let source = fixture("420-vertical-full-odd-alpha.avif");
    let admitted = avif::sdr::admit_ycbcr8(&source, avif::INSPECT_PIXELS_MAX, &|| Ok(())).unwrap();
    let primary = decode_item(&admitted.primary).unwrap();
    let alpha = decode_item(admitted.alpha.as_ref().unwrap()).unwrap();
    let count = admitted.primary.width as usize * admitted.primary.height as usize;
    for (planes, opacity) in [
        (
            &primary.planes[..primary.planes.len() - 1],
            Some(&alpha.planes[..count]),
        ),
        (primary.planes.as_slice(), Some(&alpha.planes[..count - 1])),
        (primary.planes.as_slice(), None),
    ] {
        assert_eq!(
            avif::sdr::reconstruct(&admitted, source.len() as u64, planes, opacity, &|| Ok(()))
                .unwrap_err()
                .code,
            ErrorCode::CodecFailed
        );
    }
    for cancel_at in 1..=11 {
        let calls = Cell::new(0);
        let error = avif::sdr::reconstruct(
            &admitted,
            source.len() as u64,
            &primary.planes,
            Some(&alpha.planes[..count]),
            &|| {
                calls.set(calls.get() + 1);
                if calls.get() == cancel_at {
                    Err(Error::new(ErrorCode::OperationCancelled, "cancelled"))
                } else {
                    Ok(())
                }
            },
        )
        .unwrap_err();
        assert_eq!(error.code, ErrorCode::OperationCancelled);
    }
}

#[test]
fn explicit_work_declarations_count_i32_and_native_slots_before_staging() {
    let source = fixture("420-vertical-full-odd-alpha.avif");
    let admitted = avif::sdr::admit_ycbcr8(&source, avif::INSPECT_PIXELS_MAX, &|| Ok(())).unwrap();
    let layout = avif::sdr::work_layout(&admitted, source.len() as u64).unwrap();
    assert_eq!(layout.native_planes_bytes, 17 * 13 * 3 * 2);
    assert_eq!(layout.staged_samples_bytes, (17 * 13 * 2 + 9 * 7 * 2) * 4);
    assert_eq!(layout.rgba_bytes, 17 * 13 * 4);
    assert!(layout.converter_scratch_bytes > 0);
    assert_eq!(
        layout.host_reservation_bytes,
        paintcraft_formats::codec_worker::reservation_bytes_for_input(source.len() as u64).unwrap()
    );
    assert_eq!(
        layout.require_heap(4096).unwrap_err().code,
        ErrorCode::ResourceLimit
    );
    let primary = decode_item(&admitted.primary).unwrap();
    let alpha = decode_item(admitted.alpha.as_ref().unwrap()).unwrap();
    let calls = Cell::new(0);
    let error = avif::sdr::reconstruct(
        &admitted,
        avif::INPUT_BYTES_MAX as u64 + 1,
        &primary.planes,
        Some(&alpha.planes[..17 * 13]),
        &|| {
            calls.set(calls.get() + 1);
            Ok(())
        },
    )
    .unwrap_err();
    assert_eq!(error.code, ErrorCode::ResourceLimit);
    assert_eq!(
        calls.get(),
        1,
        "invalid declared source must refuse before staging"
    );
}

#[test]
fn forged_public_declarations_refuse_before_arithmetic_or_sample_staging() {
    let source = fixture("420-vertical-full-odd-alpha.avif");
    for length in [u64::MAX, avif::INPUT_BYTES_MAX as u64 + 1, 0] {
        let admitted =
            avif::sdr::admit_ycbcr8(&source, avif::INSPECT_PIXELS_MAX, &|| Ok(())).unwrap();
        assert_eq!(
            avif::sdr::work_layout(&admitted, length).unwrap_err().code,
            ErrorCode::ResourceLimit
        );
    }
    for mutation in 0..8 {
        let mut admitted =
            avif::sdr::admit_ycbcr8(&source, avif::INSPECT_PIXELS_MAX, &|| Ok(())).unwrap();
        let primary = decode_item(&admitted.primary).unwrap();
        let alpha = decode_item(admitted.alpha.as_ref().unwrap()).unwrap();
        match mutation {
            0 => {
                admitted.primary.byte_len = usize::MAX;
                admitted.alpha.as_mut().unwrap().byte_len = usize::MAX;
            }
            1 => {
                admitted.primary.width = u32::MAX;
                admitted.primary.height = u32::MAX;
            }
            2 => admitted.primary.sequence.matrix = 0,
            3 => admitted.primary.sequence.chroma_position = 0,
            4 => admitted.primary.sequence.bit_depth = 10,
            5 => admitted.alpha.as_mut().unwrap().sequence.full_range = false,
            6 => admitted.alpha.as_mut().unwrap().width += 1,
            _ => admitted.alpha.as_mut().unwrap().sequence.monochrome = false,
        }
        let calls = Cell::new(0);
        let error = avif::sdr::reconstruct(
            &admitted,
            source.len() as u64,
            &primary.planes,
            Some(&alpha.planes[..17 * 13]),
            &|| {
                calls.set(calls.get() + 1);
                Ok(())
            },
        )
        .unwrap_err();
        assert!(matches!(
            error.code,
            ErrorCode::ResourceLimit | ErrorCode::CodecFailed
        ));
        assert_eq!(
            calls.get(),
            1,
            "mutation{mutation}: refuse before large sample staging"
        );
    }
}
