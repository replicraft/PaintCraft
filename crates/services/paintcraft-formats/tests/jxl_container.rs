//! Structural vectors wrap the existing independent libjxl codestream; the
//! pixel oracle is unchanged. Rules follow libjxl v0.12.0 decode.cc and the
//! selected jxl 0.8.0 api/box_parser.rs, including version-1 out-of-order parts.
use paintcraft_core::types::ErrorCode;
use paintcraft_formats::jxl;
const FIXTURE: &[u8] = include_bytes!("jxl-fixtures/rgba8.jxl");
fn stream() -> &'static [u8] {
    assert_eq!(&FIXTURE[36..40], b"jxlc");
    &FIXTURE[40..]
}
fn bx(kind: &[u8; 4], payload: &[u8]) -> Vec<u8> {
    let mut out = ((payload.len() + 8) as u32).to_be_bytes().to_vec();
    out.extend(kind);
    out.extend(payload);
    out
}
fn prefix(version: u32) -> Vec<u8> {
    let mut out = FIXTURE[..12].to_vec();
    let mut filetype = b"jxl ".to_vec();
    filetype.extend(version.to_be_bytes());
    filetype.extend(b"jxl ");
    out.extend(bx(b"ftyp", &filetype));
    out
}
fn fragment(index: u32, last: bool, payload: &[u8]) -> Vec<u8> {
    let mut content = (index | if last { 1 << 31 } else { 0 })
        .to_be_bytes()
        .to_vec();
    content.extend(payload);
    bx(b"jxlp", &content)
}
#[test]
fn malformed_suffix_codestreams_and_type_boxes_cannot_hide_after_a_complete_image() {
    for suffix in [
        bx(b"jxlc", stream()),
        fragment(0, true, stream()),
        bx(b"ftyp", b"jxl \0\0\0\0jxl "),
        FIXTURE[..12].to_vec(),
    ] {
        let mut bytes = FIXTURE.to_vec();
        bytes.extend(suffix);
        assert!(
            jxl::inspect_checked(&bytes, 6, &|| Ok(())).is_err(),
            "header accepted conflicting suffix"
        );
        assert!(
            jxl::decode_with_pixel_limit(&bytes, 6).is_err(),
            "decoder ignored conflicting suffix"
        );
    }
}
#[test]
fn missing_fragment_last_marker_is_rejected_even_when_pixels_are_complete() {
    let mut bytes = prefix(0);
    bytes.extend(fragment(0, false, stream()));
    assert!(jxl::decode_with_pixel_limit(&bytes, 6).is_err());
}

fn decoded(bytes: &[u8]) -> jxl::Decoded {
    jxl::decode_with_pixel_limit(bytes, 6).unwrap()
}
#[test]
fn valid_box_lengths_compatible_brands_level_and_auxiliary_positions_keep_exact_pixels() {
    let expected = decoded(FIXTURE);
    let mut bytes = FIXTURE[..12].to_vec();
    bytes.extend(bx(b"ftyp", b"jxl \0\0\0\0jxl test"));
    bytes.extend(bx(b"jxll", &[5]));
    bytes.extend(bx(b"Exif", &[0, 0, 0, 0]));
    bytes.extend(bx(b"xml ", b"<x/>").as_slice());
    let mut wide = 1u32.to_be_bytes().to_vec();
    wide.extend(b"jxlc");
    wide.extend(((stream().len() + 16) as u64).to_be_bytes());
    wide.extend(stream());
    bytes.extend(wide);
    bytes.extend(bx(b"jumb", b"opaque ignored metadata"));
    bytes.extend(bx(b"uuid", &[0; 16]));
    // Compressed empty XMP, with a valid one-byte Brotli empty stream. No
    // auxiliary data is requested or decompressed by the selected decoder.
    bytes.extend(bx(b"brob", b"xml \x3b"));
    // Size zero consumes the remaining input; valid auxiliary data may follow
    // the codestream even when the upstream pixel decoder finishes earlier.
    bytes.extend([0, 0, 0, 0, b'f', b'r', b'e', b'e', 1, 2, 3]);
    let info = jxl::inspect_checked(&bytes, 6, &|| Ok(())).unwrap();
    assert!(info.metadata_boxes);
    let actual = decoded(&bytes);
    assert_eq!(actual.samples, expected.samples);
    assert_eq!(actual.losses, ["jxl-container-metadata-ignored"]);
    let mut unbounded = prefix(0);
    unbounded.extend([0, 0, 0, 0, b'j', b'x', b'l', b'c']);
    unbounded.extend(stream());
    assert_eq!(decoded(&unbounded), expected);
}
#[test]
fn valid_partial_codestreams_include_in_order_empty_parts_and_version_one_reordering() {
    let expected = decoded(FIXTURE);
    for (version, order) in [(0, [0, 1, 2]), (1, [0, 1, 2])] {
        let mut bytes = prefix(version);
        let s = stream();
        let n = s.len() / 3;
        let parts = [&s[..n], &s[n..n * 2], &s[n * 2..]];
        for index in order {
            bytes.extend(fragment(index as u32, index == 2, parts[index]));
            bytes.extend(bx(b"free", &[]));
        }
        let actual = jxl::decode_with_pixel_limit(&bytes, 6)
            .unwrap_or_else(|error| panic!("version {version} order {order:?}: {error}"));
        assert_eq!(
            actual.samples, expected.samples,
            "version {version} order {order:?}"
        );
    }
    // A frame must start at a valid checkpoint. Keep the image and frame
    // headers in the physically first part, then reorder later pixel parts.
    let mut reordered = prefix(1);
    let s = stream();
    let n = s.len() - 12;
    let parts = [&s[..n], &s[n..n + 4], &s[n + 4..n + 8], &s[n + 8..]];
    for index in [0, 2, 1, 3] {
        reordered.extend(fragment(index as u32, index == 3, parts[index]));
    }
    assert_eq!(decoded(&reordered).samples, expected.samples);
    let mut empty = prefix(0);
    empty.extend(fragment(0, false, &[]));
    empty.extend(fragment(1, true, stream()));
    assert_eq!(decoded(&empty).samples, expected.samples);
    let mut unbounded = prefix(0);
    unbounded.extend(fragment(0, false, &stream()[..8]));
    let mut final_part = fragment(1, true, &stream()[8..]);
    final_part[..4].fill(0);
    unbounded.extend(final_part);
    assert_eq!(decoded(&unbounded).samples, expected.samples);
}
#[test]
fn fragment_headers_gaps_duplicates_mixed_streams_and_conflicting_last_flags_fail_closed() {
    for parts in [
        vec![(1, true)],
        vec![(0, false)],
        vec![(0, false), (0, true)],
        vec![(0, true), (1, true)],
        vec![(0, false), (2, true)],
        vec![(2, true), (0, true), (1, false)],
        vec![(0, true), (1, false)],
        vec![(4096, true)],
    ] {
        let mut bytes = prefix(1);
        for (index, last) in parts {
            bytes.extend(fragment(index, last, stream()));
        }
        assert!(jxl::inspect_checked(&bytes, 6, &|| Ok(())).is_err());
        assert!(jxl::decode_with_pixel_limit(&bytes, 6).is_err());
    }
    let mut out_of_order = prefix(0);
    out_of_order.extend(fragment(1, true, &stream()[8..]));
    out_of_order.extend(fragment(0, false, &stream()[..8]));
    assert!(jxl::inspect_checked(&out_of_order, 6, &|| Ok(())).is_err());
    for len in 0..4 {
        let mut bytes = prefix(0);
        bytes.extend(bx(b"jxlp", &[0; 4][..len]));
        assert!(jxl::inspect_checked(&bytes, 6, &|| Ok(())).is_err());
    }
    let mut mixed = prefix(0);
    mixed.extend(fragment(0, true, stream()));
    mixed.extend(bx(b"jxlc", stream()));
    assert!(jxl::inspect_checked(&mixed, 6, &|| Ok(())).is_err());
    let mut unfinished = prefix(0);
    let mut part = fragment(0, false, stream());
    part[..4].fill(0);
    unfinished.extend(part);
    assert!(jxl::inspect_checked(&unfinished, 6, &|| Ok(())).is_err());
}
#[test]
fn file_type_level_and_compressed_wrapper_rules_apply_before_decoder_headers() {
    let mut no_type = FIXTURE[..12].to_vec();
    no_type.extend(bx(b"jxlc", stream()));
    assert!(jxl::inspect_checked(&no_type, 6, &|| Ok(())).is_err());
    for kind in [b"ftyp", b"free"] {
        let mut empty = FIXTURE[..12].to_vec();
        empty.extend(bx(kind, &[]));
        assert!(jxl::inspect_checked(&empty, 6, &|| Ok(())).is_err());
    }
    for payload in [
        b"jxl \0\0\0\0".as_slice(),
        b"jxl \0\0\0\0jxl x",
        b"oops\0\0\0\0jxl ",
    ] {
        let mut bad = FIXTURE[..12].to_vec();
        bad.extend(bx(b"ftyp", payload));
        bad.extend(bx(b"jxlc", stream()));
        assert!(jxl::inspect_checked(&bad, 6, &|| Ok(())).is_err());
    }
    let mut future = prefix(2);
    future.extend(bx(b"jxlc", stream()));
    assert_eq!(
        jxl::inspect_checked(&future, 6, &|| Ok(()))
            .unwrap_err()
            .code,
        ErrorCode::CapabilityUnsupported
    );
    for levels in [vec![vec![]], vec![vec![5, 5]], vec![vec![5], vec![5]]] {
        let mut bad = prefix(0);
        for level in levels {
            bad.extend(bx(b"jxll", &level));
        }
        bad.extend(bx(b"jxlc", stream()));
        assert!(jxl::inspect_checked(&bad, 6, &|| Ok(())).is_err());
    }
    let mut late = FIXTURE.to_vec();
    late.extend(bx(b"jxll", &[5]));
    assert!(jxl::inspect_checked(&late, 6, &|| Ok(())).is_err());
    let mut main10 = prefix(0);
    main10.extend(bx(b"jxll", &[10]));
    main10.extend(bx(b"jxlc", stream()));
    assert_eq!(decoded(&main10), decoded(FIXTURE));
    let mut invalid_level = prefix(0);
    invalid_level.extend(bx(b"jxll", &[7]));
    invalid_level.extend(bx(b"jxlc", stream()));
    assert!(jxl::inspect_checked(&invalid_level, 6, &|| Ok(())).is_err());
    for inner in [b"brob", b"jxlc", b"jxlp", b"jbrd"] {
        let mut bad = FIXTURE.to_vec();
        bad.extend(bx(b"brob", inner));
        assert!(jxl::inspect_checked(&bad, 6, &|| Ok(())).is_err());
    }
    for len in 0..4 {
        let mut bad = FIXTURE.to_vec();
        bad.extend(bx(b"brob", &b"xml "[..len]));
        assert!(jxl::inspect_checked(&bad, 6, &|| Ok(())).is_err());
    }
}

#[test]
fn full_container_preflight_preserves_cancellation_before_any_pixel_header() {
    use paintcraft_core::types::Error;
    use std::cell::Cell;
    let mut source = prefix(0);
    for _ in 0..8 {
        source.extend(bx(b"free", &[]));
    }
    source.extend(bx(b"jxlc", stream()));
    // One initial check and one for each box; an error here must not be
    // swallowed or reclassified by subsequent selected-decoder processing.
    for stop in 1..=11 {
        let calls = Cell::new(0);
        let result = jxl::inspect_checked(&source, 6, &|| {
            calls.set(calls.get() + 1);
            if calls.get() == stop {
                Err(Error::new(ErrorCode::OperationCancelled, "preflight stop"))
            } else {
                Ok(())
            }
        });
        assert_eq!(result.unwrap_err().code, ErrorCode::OperationCancelled);
        assert_eq!(calls.get(), stop);
    }
}
