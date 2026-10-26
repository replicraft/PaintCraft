use super::*;
use std::{cell::Cell, io::Cursor, sync::Arc};
use uuid::Uuid;
const SMALL: &[u8] = include_bytes!("../../../tests/jxr-fixtures/rgb8-4x3.jxr");
const EDGE: &[u8] = include_bytes!("../../../tests/jxr-fixtures/rgb8-17x19.jxr");
fn owner() -> Owner {
    Owner {
        session: Uuid::from_u128(17),
        state: Uuid::from_u128(29),
        epoch: 7,
        frontend: 3,
    }
}
fn rgb(w: u32, h: u32) -> Vec<u8> {
    (0..h)
        .flat_map(|y| {
            (0..w).flat_map(move |x| (0..3).map(move |c| ((x * 47 + y * 13 + c * 71) % 256) as u8))
        })
        .collect()
}
fn request() -> Request {
    Request::capture(
        Layout::new(4, 3, 65_536).unwrap(),
        Arc::new(rgb(4, 3)),
        owner(),
        &|| Ok(()),
    )
    .unwrap()
}
fn response(request: &Request) -> Vec<u8> {
    let mut out = Vec::new();
    write_success(request.header(), SMALL, 123, &mut out, &|| Ok(())).unwrap();
    out
}
fn cancelled() -> Error {
    Error::new(ErrorCode::OperationCancelled, "injected cancellation")
}

#[test]
fn canonical_layout_and_request_match_exact_bytes_and_independent_file() {
    let request = request();
    let header = request.header();
    let bytes = header.encode();
    assert_eq!(&bytes[..12], b"PCENC001\0\x01\x01\0");
    assert_eq!(&bytes[12..24], &[0, 0, 0, 4, 0, 0, 0, 3, 0, 0, 0, 12]);
    assert_eq!(u64::from_be_bytes(bytes[24..32].try_into().unwrap()), 36);
    assert_eq!(
        u64::from_be_bytes(bytes[32..40].try_into().unwrap()),
        65_536
    );
    assert_eq!(Header::decode(&bytes).unwrap(), header);
    let mut canonical = bytes;
    canonical[129..161].fill(0);
    let mut independent = Sha256::new();
    independent.update(b"paintcraft-jxr-rgb8-encode-request/v1\0");
    independent.update(canonical);
    assert_eq!(
        header.request_sha256(),
        <[u8; 32]>::from(independent.finalize())
    );
    assert_eq!(
        header.rgb_sha256(),
        <[u8; 32]>::from(Sha256::digest(rgb(4, 3)))
    );
    assert!(bytes[161..].iter().all(|n| *n == 0));
    let mut wire = Vec::new();
    request.write_to(&mut wire, &|| Ok(())).unwrap();
    assert_eq!(wire.len(), REQUEST_HEADER + 36);
    let read = Request::read_from(&mut Cursor::new(wire), &|| Ok(())).unwrap();
    assert_eq!(read.header(), header);
    assert_eq!(read.rgb(), request.rgb());
    let output = response(&request);
    let frame = decode_response(&output, header, owner(), &|| Ok(())).unwrap();
    assert_eq!(frame.bytes(), SMALL);
    assert_eq!(frame.sha256(), <[u8; 32]>::from(Sha256::digest(SMALL)));
    assert_eq!(frame.heap_peak(), 123);
    assert_eq!(frame.inspection().resolution_x_bits, Some(0));
    assert_eq!(frame.inspection().resolution_y_bits, Some(0));
    assert_eq!(frame.profile(), PROFILE);
    assert_eq!(frame.density_policy(), DENSITY_POLICY);
}
#[test]
fn supervised_segments_retain_the_same_immutable_rgb_allocation() {
    let source = Arc::new(rgb(4, 3));
    let request = Request::capture(
        Layout::new(4, 3, 65_536).unwrap(),
        source.clone(),
        owner(),
        &|| Ok(()),
    )
    .unwrap();
    let segments = request.segments();
    assert_eq!(segments.len(), 2);
    assert_eq!(segments[0].as_slice(), request.header().encode());
    assert!(Arc::ptr_eq(&segments[1], &source));
    drop(request);
    assert_eq!(segments[1].as_slice(), source.as_slice());
}
#[test]
fn every_request_header_byte_is_canonical_and_source_extent_is_exact() {
    let request = request();
    let original = request.header().encode();
    for index in 0..REQUEST_HEADER {
        let mut changed = original;
        changed[index] ^= 1;
        assert!(Header::decode(&changed).is_err(), "changed byte {index}");
    }
    for length in 0..REQUEST_HEADER {
        assert!(Header::decode(&original[..length]).is_err());
    }
    let mut extra = original.to_vec();
    extra.push(0);
    assert!(Header::decode(&extra).is_err());
    for source in [vec![0; 35], vec![0; 37], vec![0; 48], vec![0; 36]] {
        assert!(Request::from_parts(request.header(), Arc::new(source), &|| Ok(())).is_err());
    }
    let mut wire = Vec::new();
    request.write_to(&mut wire, &|| Ok(())).unwrap();
    for length in 0..wire.len() {
        assert!(
            Request::read_from(&mut Cursor::new(&wire[..length]), &|| Ok(())).is_err(),
            "prefix {length}"
        );
    }
    // Header-only overflow declarations fail before a body read/allocation.
    let mut declared = original;
    declared[24..32].copy_from_slice(&u64::MAX.to_be_bytes());
    let mut reader = Cursor::new(declared);
    assert!(Request::read_from(&mut reader, &|| Ok(())).is_err());
    assert_eq!(reader.position(), REQUEST_HEADER as u64);
}
#[test]
fn fixed_policy_limits_and_quote_are_checked_without_pixel_allocation() {
    for (w, h, cap) in [
        (0, 1, 1),
        (1, 0, 1),
        (65_537, 1, 1),
        (1, 65_537, 1),
        (1025, 1024, 1),
        (u32::MAX, u32::MAX, 1),
        (1, 1, 0),
        (1, 1, ENCODED_BYTES_MAX + 1),
        (1, 1, u64::MAX),
    ] {
        assert!(Layout::new(w, h, cap).is_err());
    }
    let layout = Layout::new(65_536, 16, ENCODED_BYTES_MAX).unwrap();
    assert_eq!(layout.rgb_bytes(), RGB_BYTES_MAX);
    assert_eq!(layout.stride(), 65_536 * 3);
    assert_eq!(
        layout.reservation_bytes().unwrap(),
        HEAP_BYTES
            + RGB_BYTES_MAX
            + REQUEST_HEADER as u64
            + ENCODED_BYTES_MAX
            + (RESPONSE_HEADER + VERSION_BYTES_MAX + DETAIL_BYTES_MAX) as u64
            + CHUNK as u64
    );
    let mut classic = owner();
    classic.frontend = 1;
    assert!(
        Request::capture(
            Layout::new(4, 3, 1000).unwrap(),
            Arc::new(rgb(4, 3)),
            classic,
            &|| Ok(())
        )
        .is_err()
    );
    // No RGBA entrypoint or implicit discard: a 4-byte/pixel buffer is not RGB.
    assert!(
        Request::capture(
            Layout::new(4, 3, 1000).unwrap(),
            Arc::new(vec![255; 48]),
            owner(),
            &|| Ok(())
        )
        .is_err()
    );
}
#[test]
fn nonce_owner_cap_and_source_bind_distinct_jobs() {
    let first = request();
    let output = response(&first);
    let second = request();
    assert_ne!(first.header().nonce(), second.header().nonce());
    assert_ne!(
        first.header().request_sha256(),
        second.header().request_sha256()
    );
    assert_eq!(
        decode_response(&output, second.header(), owner(), &|| Ok(()))
            .unwrap_err()
            .code,
        ErrorCode::ConflictStaleBase
    );
    let mut changed_owner = owner();
    changed_owner.epoch += 1;
    let different = Request::capture(
        Layout::new(4, 3, 65_536).unwrap(),
        Arc::new(rgb(4, 3)),
        changed_owner,
        &|| Ok(()),
    )
    .unwrap();
    assert!(decode_response(&output, different.header(), changed_owner, &|| Ok(())).is_err());
    assert!(decode_response(&output, first.header(), changed_owner, &|| Ok(())).is_err());
    let different_cap = Request::capture(
        Layout::new(4, 3, 65_537).unwrap(),
        Arc::new(rgb(4, 3)),
        owner(),
        &|| Ok(()),
    )
    .unwrap();
    assert!(decode_response(&output, different_cap.header(), owner(), &|| Ok(())).is_err());
    let different_rgb = Request::capture(
        Layout::new(4, 3, 65_536).unwrap(),
        Arc::new(vec![0; 36]),
        owner(),
        &|| Ok(()),
    )
    .unwrap();
    assert!(decode_response(&output, different_rgb.header(), owner(), &|| Ok(())).is_err());
}
#[test]
fn same_nonce_still_binds_cap_geometry_and_raw_source_policy() {
    fn rebind(mut encoded: [u8; REQUEST_HEADER]) -> Header {
        encoded[129..161].fill(0);
        let mut hash = Sha256::new();
        hash.update(b"paintcraft-jxr-rgb8-encode-request/v1\0");
        hash.update(encoded);
        encoded[129..161].copy_from_slice(&hash.finalize());
        Header::decode(&encoded).unwrap()
    }
    let original = request();
    let output = response(&original);
    let bytes = original.header().encode();
    let mut cap = bytes;
    cap[32..40].copy_from_slice(&65_537u64.to_be_bytes());
    let mut geometry = bytes;
    geometry[12..16].copy_from_slice(&3u32.to_be_bytes());
    geometry[16..20].copy_from_slice(&4u32.to_be_bytes());
    geometry[20..24].copy_from_slice(&9u32.to_be_bytes());
    let mut source = bytes;
    source[97..129].copy_from_slice(&Sha256::digest([0; 36]));
    for changed in [cap, geometry, source] {
        let header = rebind(changed);
        assert_eq!(header.nonce(), original.header().nonce());
        assert_eq!(header.owner(), original.header().owner());
        assert_ne!(header.request_sha256(), original.header().request_sha256());
        assert_eq!(
            decode_response(&output, header, owner(), &|| Ok(()))
                .unwrap_err()
                .code,
            ErrorCode::ConflictStaleBase
        );
    }
}
#[test]
fn response_rejects_bindings_bounds_truncation_extra_and_pixel_corruption() {
    let request = request();
    let original = response(&request);
    for index in [
        0,
        8,
        10,
        11,
        12,
        16,
        36,
        77,
        93,
        125,
        157,
        189,
        191,
        193,
        207,
        RESPONSE_HEADER,
    ] {
        let mut changed = original.clone();
        changed[index] ^= 1;
        assert!(
            decode_response(&changed, request.header(), owner(), &|| Ok(())).is_err(),
            "field byte {index}"
        );
    }
    for (range, value) in [(20..28, u64::MAX), (28..36, HEAP_BYTES + 1)] {
        let mut changed = original.clone();
        changed[range].copy_from_slice(&value.to_be_bytes());
        assert!(decode_response(&changed, request.header(), owner(), &|| Ok(())).is_err());
    }
    for length in 0..original.len() {
        assert!(
            decode_response(&original[..length], request.header(), owner(), &|| Ok(())).is_err(),
            "response prefix {length}"
        );
    }
    let mut extra = original.clone();
    extra.push(0);
    assert!(decode_response(&extra, request.header(), owner(), &|| Ok(())).is_err());
    let mut changed = original;
    *changed.last_mut().unwrap() ^= 1;
    assert!(decode_response(&changed, request.header(), owner(), &|| Ok(())).is_err());
}
#[test]
fn encoder_file_profile_rejects_changed_density_version_extent_and_geometry_before_write() {
    let request = request();
    let ifd = u32::from_le_bytes(SMALL[4..8].try_into().unwrap()) as usize;
    let count = u16::from_le_bytes(SMALL[ifd..ifd + 2].try_into().unwrap()) as usize;
    let density = (0..count)
        .map(|i| ifd + 2 + i * 12)
        .find(|p| u16::from_le_bytes(SMALL[*p..*p + 2].try_into().unwrap()) == 0xbc82)
        .unwrap();
    for bits in [96f32.to_bits(), (-0f32).to_bits()] {
        let mut changed = SMALL.to_vec();
        changed[density + 8..density + 12].copy_from_slice(&bits.to_le_bytes());
        let mut sink = Vec::new();
        assert!(write_success(request.header(), &changed, 0, &mut sink, &|| Ok(())).is_err());
        assert!(sink.is_empty());
    }
    let mut version = SMALL.to_vec();
    version[3] = 0;
    let mut extra = SMALL.to_vec();
    extra.push(0);
    for encoded in [version.as_slice(), extra.as_slice(), EDGE, &[0; 48]] {
        let mut sink = Vec::new();
        assert!(write_success(request.header(), encoded, 0, &mut sink, &|| Ok(())).is_err());
        assert!(sink.is_empty());
    }
    let tiny = Request::capture(
        Layout::new(4, 3, (SMALL.len() - 1) as u64).unwrap(),
        Arc::new(rgb(4, 3)),
        owner(),
        &|| Ok(()),
    )
    .unwrap();
    assert!(write_success(tiny.header(), SMALL, 0, &mut Vec::new(), &|| Ok(())).is_err());
}
#[test]
fn error_frames_are_bounded_and_cannot_smuggle_output() {
    let request = request();
    for code in [
        ErrorCode::CodecFailed,
        ErrorCode::ResourceLimit,
        ErrorCode::OperationCancelled,
        ErrorCode::CapabilityUnsupported,
    ] {
        let mut out = Vec::new();
        write_failure(
            request.header(),
            &Error::new(code, "失敗".repeat(2000)),
            17,
            &mut out,
            &|| Ok(()),
        )
        .unwrap();
        let error = decode_response(&out, request.header(), owner(), &|| Ok(())).unwrap_err();
        assert_eq!(error.code, code);
        assert!(error.message.len() <= DETAIL_BYTES_MAX);
        let mut smuggled = out.clone();
        smuggled[12..16].copy_from_slice(&4u32.to_be_bytes());
        assert!(decode_response(&smuggled, request.header(), owner(), &|| Ok(())).is_err());
        let mut bad_utf8 = out;
        *bad_utf8.last_mut().unwrap() = 0xff;
        assert!(decode_response(&bad_utf8, request.header(), owner(), &|| Ok(())).is_err());
    }
    let mut sink = Vec::new();
    assert!(
        write_failure(
            request.header(),
            &cancelled(),
            HEAP_BYTES + 1,
            &mut sink,
            &|| Ok(())
        )
        .is_err()
    );
    assert!(sink.is_empty());
}
#[test]
fn decode_and_encode_frames_cannot_be_interchanged() {
    use crate::codec_worker as old;
    let encode = request();
    assert!(old::Header::decode(&encode.header().encode()[..old::REQUEST_HEADER]).is_err());
    let decode =
        old::Request::capture_for(old::Codec::JxrRgb8, Arc::new(SMALL.to_vec()), owner(), 12)
            .unwrap();
    assert!(Header::decode(&decode.header.encode()).is_err());
    let old_output = old::success(
        decode.header,
        old::Success {
            width: 4,
            height: 3,
            pixels: old::Pixels::SrgbRgba8(&[0; 48]),
        },
        0,
    )
    .unwrap();
    assert!(decode_response(&old_output, encode.header(), owner(), &|| Ok(())).is_err());
    assert!(old::decode(&response(&encode), &decode, owner()).is_err());
}
struct InterruptIo<'a> {
    cursor: Cursor<Vec<u8>>,
    cancel: &'a Cell<bool>,
}
impl Read for InterruptIo<'_> {
    fn read(&mut self, out: &mut [u8]) -> std::io::Result<usize> {
        let count = self.cursor.read(out)?;
        self.cancel.set(true);
        Ok(count)
    }
}
impl Write for InterruptIo<'_> {
    fn write(&mut self, bytes: &[u8]) -> std::io::Result<usize> {
        let count = self.cursor.write(bytes)?;
        self.cancel.set(true);
        Ok(count)
    }
    fn flush(&mut self) -> std::io::Result<()> {
        Ok(())
    }
}
#[test]
fn cancellation_and_io_failure_never_return_a_complete_private_frame() {
    let request = request();
    assert_eq!(
        Request::capture(
            request.header().layout(),
            Arc::new(rgb(4, 3)),
            owner(),
            &|| Err(cancelled())
        )
        .unwrap_err()
        .code,
        ErrorCode::OperationCancelled
    );
    let cancel = Cell::new(false);
    let check = || {
        if cancel.get() {
            Err(cancelled())
        } else {
            Ok(())
        }
    };
    let mut writer = InterruptIo {
        cursor: Cursor::new(Vec::new()),
        cancel: &cancel,
    };
    assert_eq!(
        request.write_to(&mut writer, &check).unwrap_err().code,
        ErrorCode::OperationCancelled
    );
    assert_eq!(writer.cursor.get_ref().len(), REQUEST_HEADER);
    cancel.set(false);
    let mut bytes = Vec::new();
    request.write_to(&mut bytes, &|| Ok(())).unwrap();
    let mut reader = InterruptIo {
        cursor: Cursor::new(bytes),
        cancel: &cancel,
    };
    assert_eq!(
        Request::read_from(&mut reader, &check).unwrap_err().code,
        ErrorCode::OperationCancelled
    );
    assert_eq!(reader.cursor.position(), REQUEST_HEADER as u64);
    cancel.set(false);
    writer.cursor = Cursor::new(Vec::new());
    assert_eq!(
        write_success(request.header(), SMALL, 0, &mut writer, &check)
            .unwrap_err()
            .code,
        ErrorCode::OperationCancelled
    );
    assert_eq!(writer.cursor.get_ref().len(), RESPONSE_HEADER);
    assert_eq!(
        decode_response(&response(&request), request.header(), owner(), &|| Err(
            cancelled()
        ))
        .unwrap_err()
        .code,
        ErrorCode::OperationCancelled
    );
    struct FailedWriter;
    impl Write for FailedWriter {
        fn write(&mut self, _: &[u8]) -> std::io::Result<usize> {
            Err(std::io::Error::other("injected"))
        }
        fn flush(&mut self) -> std::io::Result<()> {
            Ok(())
        }
    }
    assert!(request.write_to(&mut FailedWriter, &|| Ok(())).is_err());
    assert!(write_success(request.header(), SMALL, 0, &mut FailedWriter, &|| Ok(())).is_err());
}

#[test]
fn interrupted_io_polls_cancellation_before_retrying() {
    struct Interrupted<'a>(&'a Cell<bool>);
    impl Read for Interrupted<'_> {
        fn read(&mut self, _: &mut [u8]) -> std::io::Result<usize> {
            self.0.set(true);
            Err(std::io::ErrorKind::Interrupted.into())
        }
    }
    impl Write for Interrupted<'_> {
        fn write(&mut self, _: &[u8]) -> std::io::Result<usize> {
            self.0.set(true);
            Err(std::io::ErrorKind::Interrupted.into())
        }
        fn flush(&mut self) -> std::io::Result<()> {
            Ok(())
        }
    }
    let cancel = Cell::new(false);
    let check = || {
        if cancel.get() {
            Err(cancelled())
        } else {
            Ok(())
        }
    };
    assert_eq!(
        Request::read_from(&mut Interrupted(&cancel), &check)
            .unwrap_err()
            .code,
        ErrorCode::OperationCancelled
    );
    cancel.set(false);
    assert_eq!(
        request()
            .write_to(&mut Interrupted(&cancel), &check)
            .unwrap_err()
            .code,
        ErrorCode::OperationCancelled
    );
}

#[test]
fn fresh_long_header_response_is_validated_without_enabling_reader() {
    for (bytes, w, h) in [
        (
            include_bytes!("../../../tests/jxr-fixtures/white-rgb8-4096x1.jxr").as_slice(),
            4096,
            1,
        ),
        (
            include_bytes!("../../../tests/jxr-fixtures/white-rgb8-1x4096.jxr").as_slice(),
            1,
            4096,
        ),
    ] {
        let request = Request::capture(
            Layout::new(w, h, ENCODED_BYTES_MAX).unwrap(),
            Arc::new(vec![255; w as usize * h as usize * 3]),
            owner(),
            &|| Ok(()),
        )
        .unwrap();
        let mut output = Vec::new();
        write_success(request.header(), bytes, 123, &mut output, &|| Ok(())).unwrap();
        let frame = decode_response(&output, request.header(), owner(), &|| Ok(())).unwrap();
        assert_eq!(frame.bytes(), bytes);
        assert_eq!(
            (frame.inspection().width, frame.inspection().height),
            (w, h)
        );
        assert_eq!(
            crate::jxr::inspect_checked(bytes, PIXELS_MAX, &|| Ok(()))
                .unwrap_err()
                .code,
            ErrorCode::CapabilityUnsupported
        );
    }
}
