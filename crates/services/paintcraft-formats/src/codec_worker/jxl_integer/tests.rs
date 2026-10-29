use super::*;
fn request(codec: Codec) -> Request {
    Request::capture_for(
        codec,
        Arc::new(b"captured-jxl".to_vec()),
        Owner {
            session: Uuid::new_v4(),
            state: Uuid::new_v4(),
            epoch: 4,
            frontend: 3,
        },
        4,
    )
    .unwrap()
}
fn report(request: &Request, depth: u8, rgba: &[u8]) -> Report {
    Report {
        source_depth: depth,
        width: 2,
        height: 1,
        source_sha256: request.header.sha256,
        output_sha256: Sha256::digest(rgba).into(),
        changed_rgb_components: u32::from(depth == 16),
        changed_alpha: u32::from(depth == 16),
    }
}
fn response(request: &Request, value: Report, rgba: &[u8]) -> Vec<u8> {
    let version = env!("CARGO_PKG_VERSION").as_bytes();
    let mut out = response_header(
        request.header,
        0,
        2,
        1,
        NativeEncoding {
            primaries: 1,
            transfer: 13,
            matrix: 0,
            full_range: true,
        },
        rgba.len(),
        0,
        REPORT_BYTES,
        version.len(),
        4096,
    )
    .unwrap();
    out.extend_from_slice(version);
    out.extend_from_slice(&value.encode().unwrap());
    out.extend_from_slice(rgba);
    out
}
#[test]
fn integer_requests_remain_captured_and_bound_on_native_and_wasm() {
    let captured = request(Codec::JxlIntegerRgba8);
    let header = Header::decode(&captured.header.encode()).unwrap();
    assert_eq!(header, captured.header);
    header.verify_source(captured.source.as_slice()).unwrap();
    let mut changed = captured.source.as_ref().clone();
    changed[0] ^= 1;
    assert!(header.verify_source(&changed).is_err());
    let mut owner = captured.header.owner;
    owner.frontend = 0;
    assert!(Request::capture_for(Codec::JxlIntegerRgba8, captured.source, owner, 4).is_err());
}

#[test]
fn typed_reports_bind_both_depths_all_fields_and_actual_output() {
    let request = request(Codec::JxlIntegerRgba8);
    let rgba = [7, 8, 9, 0, 10, 11, 12, 128];
    for depth in [8, 16] {
        let value = report(&request, depth, &rgba);
        assert_eq!(Report::decode(&value.encode().unwrap()).unwrap(), value);
        let output = response(&request, value, &rgba);
        let frame = decode(&output, &request, request.header.owner).unwrap();
        assert_eq!(frame.jxl_report, Some(value));
        assert_eq!(value.losses().is_empty(), depth == 8);
        assert_eq!(frame.float_report, None);
        for at in [0, 8, 10, 11, 15, 19, 20, 52, 84, 88, 92, 94] {
            let mut corrupted = output.clone();
            corrupted[RESPONSE_HEADER + env!("CARGO_PKG_VERSION").len() + at] ^= 0xff;
            assert!(
                decode(&corrupted, &request, request.header.owner).is_err(),
                "field byte {at}"
            );
        }
        let mut corrupted = output.clone();
        *corrupted.last_mut().unwrap() ^= 1;
        assert!(decode(&corrupted, &request, request.header.owner).is_err());
        assert!(decode(&output[..output.len() - 1], &request, request.header.owner).is_err());
        let mut foreign = request.header.owner;
        foreign.epoch += 1;
        assert_eq!(
            decode(&output, &request, foreign).unwrap_err().code,
            ErrorCode::ConflictStaleBase
        );
    }
}
#[test]
fn old_jxl_tag_never_accepts_new_report_and_depth16_loss_is_mandatory() {
    let request = request(Codec::JxlRgba8);
    let rgba = [0; 8];
    let mut value = report(&request, 16, &rgba);
    value.changed_rgb_components = 0;
    value.changed_alpha = 0;
    assert_eq!(value.losses(), &[crate::jxl::DEPTH16_LOSS]);
    assert!(
        decode(
            &response(&request, value, &rgba),
            &request,
            request.header.owner
        )
        .is_err()
    );
    let mut bytes = value.encode().unwrap();
    bytes[92..94].fill(0);
    assert!(Report::decode(&bytes).is_err());
    bytes = value.encode().unwrap();
    bytes[10] = 8;
    assert!(Report::decode(&bytes).is_err());
}
