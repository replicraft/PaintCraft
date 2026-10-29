use super::*;
fn owner() -> Owner {
    Owner {
        session: Uuid::from_u128(1),
        state: Uuid::from_u128(2),
        epoch: 3,
        frontend: 3,
    }
}
fn header(selected: bool) -> Header {
    let source = [31; 128];
    let sha = digest(&source);
    Header::capture(
        &source,
        owner(),
        Uuid::from_u128(4),
        PIXELS_MAX,
        selected.then_some(Selection {
            page: Page {
                index: 1,
                width: 1,
                height: 1,
                ifd_offset: 64,
                supported: true,
                orientation: 1,
            },
            page_count: 2,
            source_sha256: sha,
        }),
    )
    .unwrap()
}
#[test]
fn typed_whole_source_request_never_reuses_decoder_or_owner_page_fields() {
    for selected in [false, true] {
        let value = header(selected);
        let bytes = value.encode();
        assert_eq!(Header::decode(&bytes).unwrap(), value);
        value.verify_source(&[31; 128]).unwrap();
        assert!(value.verify_source(&[32; 128]).is_err());
        assert!(value.verify_source(&[31; 127]).is_err());
        for magic in [
            b"PCDEC002",
            b"PCDDSQ02",
            b"PCDDSQ01",
            b"PCMSPQ02",
            b"PCMSPQ01",
        ] {
            let mut bad = bytes;
            bad[..8].copy_from_slice(magic);
            assert!(Header::decode(&bad).is_err());
        }
        for at in [11, 14, 15, 167, 175] {
            let mut bad = bytes;
            bad[at] ^= 1;
            assert!(Header::decode(&bad).is_err());
        }
        let mut bad = bytes;
        bad[10] = 3;
        assert!(Header::decode(&bad).is_err());
    }
    let mut value = header(true);
    value.selection.as_mut().unwrap().source_sha256[0] ^= 1;
    assert!(Header::decode(&value.encode()).is_err());
    let mut value = header(true);
    value.selection.as_mut().unwrap().page.ifd_offset = 0;
    assert!(Header::decode(&value.encode()).is_err());
}

#[test]
fn selected_orientation_is_bound_and_only_the_exact_normalized_geometry_is_accepted() {
    for orientation in 1..=8 {
        let mut request = header(true);
        let page = &mut request.selection.as_mut().unwrap().page;
        page.width = 3;
        page.height = 2;
        page.orientation = orientation;
        assert_eq!(Header::decode(&request.encode()).unwrap(), request);
        let (width, height) = request.selection.unwrap().page.output_dimensions();
        let pixels = vec![37; 24];
        assert!(
            response(
                &request,
                "0.1.1",
                2,
                width,
                height,
                &[0; REPORT_BYTES],
                &pixels
            )
            .is_ok()
        );
        if orientation >= 5 {
            assert!(response(&request, "0.1.1", 2, 3, 2, &[0; REPORT_BYTES], &pixels).is_err());
        }
        let mut legacy = request.encode();
        legacy[..8].copy_from_slice(b"PCTIF001");
        legacy[8..10].copy_from_slice(&1u16.to_be_bytes());
        assert!(Header::decode(&legacy).is_err());
    }
    for orientation in [0, 9, u32::MAX] {
        let mut request = header(true);
        request.selection.as_mut().unwrap().page.orientation = orientation;
        assert!(Header::decode(&request.encode()).is_err());
    }
}
#[test]
fn inventory_keeps_unsupported_first_page_without_implying_a_selection() {
    let header = header(false);
    let pages = [
        Page {
            index: 0,
            width: 0,
            height: 0,
            ifd_offset: 8,
            supported: false,
            orientation: 1,
        },
        Page {
            index: 1,
            width: 1,
            height: 1,
            ifd_offset: 64,
            supported: true,
            orientation: 1,
        },
    ];
    let payload = encode_inventory(&pages).unwrap();
    assert_eq!(decode_inventory(&payload, 2, 128).unwrap(), pages);
    let bytes = response(&header, "0.1.1", 2, 0, 0, &[], &payload).unwrap();
    assert!(matches!(
        decode_response(&bytes, &header, owner(), "0.1.1").unwrap(),
        Frame::Success { page_count: 2, .. }
    ));
    assert!(header.selection.is_none());
    assert_eq!(header.page_index(), u16::MAX);
    for at in [0, 2, 3, 12, 24, 44] {
        let mut bad = payload.clone();
        bad[at] ^= 255;
        assert!(decode_inventory(&bad, 2, 128).is_err());
    }
    assert!(decode_inventory(&payload, 1, 128).is_err());
}
#[test]
fn every_response_byte_is_bound_before_any_report_or_pixels_can_be_used() {
    for selected in [false, true] {
        let header = header(selected);
        let bytes = if selected {
            response(
                &header,
                "0.1.1",
                2,
                1,
                1,
                &[0; REPORT_BYTES],
                &[17, 29, 91, 0],
            )
            .unwrap()
        } else {
            response(
                &header,
                "0.1.1",
                1,
                0,
                0,
                &[],
                &encode_inventory(&[Page {
                    index: 0,
                    width: 1,
                    height: 1,
                    ifd_offset: 8,
                    supported: true,
                    orientation: 1,
                }])
                .unwrap(),
            )
            .unwrap()
        };
        decode_response(&bytes, &header, owner(), "0.1.1").unwrap();
        for at in 0..bytes.len() {
            let mut bad = bytes.clone();
            bad[at] ^= 1;
            assert!(
                decode_response(&bad, &header, owner(), "0.1.1").is_err(),
                "unbound byte {at} selected={selected}"
            );
        }
        let mut stale = owner();
        stale.epoch += 1;
        assert!(decode_response(&bytes, &header, stale, "0.1.1").is_err());
        assert!(decode_response(&bytes, &header, owner(), "other-version").is_err());
        let mut extra = bytes.clone();
        extra.push(0);
        assert!(decode_response(&extra, &header, owner(), "0.1.1").is_err());
        assert!(decode_response(&bytes[..bytes.len() - 1], &header, owner(), "0.1.1").is_err());
    }
}
#[test]
fn typed_completed_refusal_cannot_carry_an_inventory_or_image() {
    let header = header(true);
    for code in [
        ErrorCode::CapabilityUnsupported,
        ErrorCode::ResourceLimit,
        ErrorCode::OperationCancelled,
        ErrorCode::ConflictStaleBase,
        ErrorCode::CodecFailed,
    ] {
        let bytes = refusal(&header, "0.1.1", &Error::new(code, "test")).unwrap();
        let Frame::Refused(error) = decode_response(&bytes, &header, owner(), "0.1.1").unwrap()
        else {
            panic!("refusal expected")
        };
        assert_eq!(error.code, code);
        let mut bad = bytes;
        bad[14] = 1;
        assert!(decode_response(&bad, &header, owner(), "0.1.1").is_err());
    }
}

#[test]
fn narrow_pages_above_65536_follow_the_shared_canvas_side_and_pixel_limits() {
    assert_eq!(SIDE_MAX, 1_048_576);
    for (width, height) in [(65537, 1), (1, 65537), (SIDE_MAX, 1), (1, SIDE_MAX)] {
        let mut selected = header(true);
        let page = &mut selected.selection.as_mut().unwrap().page;
        page.width = width;
        page.height = height;
        assert_eq!(Header::decode(&selected.encode()).unwrap(), selected);
        let inventory = encode_inventory(&[Page {
            index: 0,
            width,
            height,
            ifd_offset: 8,
            supported: true,
            orientation: 1,
        }])
        .unwrap();
        assert_eq!(
            decode_inventory(&inventory, 1, 128).unwrap()[0].width,
            width
        );
    }
    for (width, height) in [(SIDE_MAX + 1, 1), (1, SIDE_MAX + 1), (SIDE_MAX, 5)] {
        let mut selected = header(true);
        let page = &mut selected.selection.as_mut().unwrap().page;
        page.width = width;
        page.height = height;
        assert!(Header::decode(&selected.encode()).is_err());
    }
}
