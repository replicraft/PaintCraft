use super::*;
use std::cell::Cell;
const XML: &str = r#"<pdnImage width="3" height="2" layers="1" savedWithVersion="3.36.3158.38068"><custom /></pdnImage>"#;
fn nrbf() -> Vec<u8> {
    let mut bytes = vec![0, 1, 0];
    for i in [1i32, -1, 1, 0] {
        bytes.extend_from_slice(&i.to_le_bytes());
    }
    bytes
}
fn file(xml: &str, body: &[u8]) -> Vec<u8> {
    let len = xml.len();
    assert!(len < 1 << 24);
    let mut bytes = b"PDN3".to_vec();
    bytes.extend_from_slice(&[
        (len & 255) as u8,
        ((len >> 8) & 255) as u8,
        ((len >> 16) & 255) as u8,
    ]);
    bytes.extend_from_slice(xml.as_bytes());
    bytes.extend_from_slice(body);
    bytes
}
fn read(bytes: &[u8]) -> Result<Inspection<'_>> {
    inspect(bytes, &Limits::default(), &|| Ok(()))
}
#[test]
fn neutral_hints_and_borrowed_spans_match_documented_historical_envelope() {
    let body = nrbf();
    let bytes = file(XML, &body);
    let parsed = read(&bytes).unwrap();
    assert_eq!(
        (parsed.hints.width, parsed.hints.height, parsed.hints.layers),
        (3, 2, 1)
    );
    assert_eq!(
        parsed.hints.saved_with_version.as_deref(),
        Some("3.36.3158.38068")
    );
    assert_eq!(parsed.xml, XML);
    assert_eq!(parsed.xml_range, 7..7 + XML.len());
    assert_eq!(parsed.body_offset, 7 + XML.len());
    assert_eq!(parsed.body, body);
    assert_eq!(
        parsed.body_header,
        BodyHeader::DeferredNrbf(NrbfHeader {
            root_id: 1,
            header_id: -1,
            major_version: 1,
            minor_version: 0
        })
    );
    assert!(parsed.hints.contains_child_elements);
    assert!(!parsed.hints.contains_unknown_root_attributes);
}
#[test]
fn utf8_length_counts_bytes_and_custom_data_is_inert() {
    let xml = r#"<?xml version="1.0" encoding="UTF-8"?><pdnImage width="&#51;" height="2" layers="0" savedWithVersion="日本語 &amp; español"><custom><名前 value="&lt;safe&gt;">&#x65e5; &amp; harmless<![CDATA[<data/>]]></名前></custom></pdnImage>"#;
    let bytes = file(xml, &nrbf());
    let parsed = read(&bytes).unwrap();
    assert_eq!(parsed.hints.width, 3);
    assert_eq!(parsed.hints.layers, 0);
    assert_eq!(
        parsed.hints.saved_with_version.as_deref(),
        Some("日本語 & español")
    );
    assert_eq!(parsed.body_offset, 7 + xml.len());
    let self_closing = file(
        r#"<pdnImage width="1" height="1" layers="1" arbitrary="retained in xml"/>"#,
        &nrbf(),
    );
    let parsed = read(&self_closing).unwrap();
    assert!(parsed.hints.contains_unknown_root_attributes);
    assert!(!parsed.hints.contains_child_elements);
}
#[test]
fn signature_variants_and_ambiguous_legacy_gzip_are_not_mislabeled() {
    for end in 0..4 {
        assert_eq!(
            probe(&b"PDN3"[..end]),
            Probe::NeedMoreInput {
                additional: 4 - end
            }
        );
    }
    assert_eq!(probe(b"PDN3"), Probe::Pdn3);
    assert_eq!(probe(b"PDN4garbage"), Probe::UnsupportedVariant(*b"PDN4"));
    assert_eq!(probe(&[0x1f, 0x8b, 8, 0]), Probe::NotPdn);
    assert_eq!(
        read(b"PDN4").unwrap_err().code,
        ErrorCode::CapabilityUnsupported
    );
    assert!(read(b"\x89PNG").is_err());
}
#[test]
fn every_header_or_nrbf_header_truncation_is_rejected() {
    let bytes = file(XML, &nrbf());
    for end in 0..bytes.len() {
        assert!(
            read(&bytes[..end]).is_err(),
            "accepted truncated header at {end}"
        );
    }
    let mut wrong = bytes;
    wrong[7 + XML.len() + 2] = 5;
    assert_eq!(read(&wrong).unwrap_err().code, ErrorCode::CodecFailed);
    let mut unsupported = file(XML, &nrbf());
    unsupported[7 + XML.len() + 2 + 9] = 2;
    assert_eq!(
        read(&unsupported).unwrap_err().code,
        ErrorCode::CapabilityUnsupported
    );
}
#[test]
fn malformed_xml_cannot_supply_document_hints() {
    for xml in [
        "",
        r#"<pdnImage width="1" height="1"/>"#,
        r#"<pdnImage width="1" width="2" height="1" layers="1"/>"#,
        r#"<pdnImage width="1" height="1" layers="1"><a></b></pdnImage>"#,
        r#"<pdnImage width="1" height="1" layers="1"/><another/>"#,
        r#"<pdnImage width="1" height="1" layers="1">&unknown;</pdnImage>"#,
        r#"<pdnImage width="1" height="1" layers="1" unknown="&missing;"/>"#,
        r#"<pdnImage width="1" height="1" layers="1"><1invalid/></pdnImage>"#,
        r#"<pdnImage width="1" height="1" layers="1" unknown="<raw"/>"#,
        r#"<pdnImage width="1" height="1" layers="1"><!-- broken -- comment --></pdnImage>"#,
        r#"<pdnImage width="1" height="1" layers="1">]]></pdnImage>"#,
        r#"<pdnImage width="0" height="1" layers="1"/>"#,
        r#"<pdnImage width="-1" height="1" layers="1"/>"#,
        r#"<pdnImage width="4294967296" height="1" layers="1"/>"#,
        r#"<pdnImage width="1" height="1" layers="1" unknown="&#0;"/>"#,
        r#"<pdnImage width="1" height="1" layers="1">&#0;</pdnImage>"#,
        "<pdnImage width=\"1\" height=\"1\" layers=\"1\">\u{1}</pdnImage>",
        r#" <?xml version="1.0"?><pdnImage width="1" height="1" layers="1"/>"#,
        r#"<?xml version="1.0" standalone="garbage"?><pdnImage width="1" height="1" layers="1"/>"#,
        r#"<?xml version="1.0" nonsense="x"?><pdnImage width="1" height="1" layers="1"/>"#,
        r#"<?xml version="1.0" version="1.0"?><pdnImage width="1" height="1" layers="1"/>"#,
    ] {
        assert!(read(&file(xml, &nrbf())).is_err(), "accepted {xml:?}");
    }
    let mut bytes = file(XML, &nrbf());
    bytes[8] = 255;
    assert!(read(&bytes).is_err());
}
#[test]
fn unsafe_xml_and_unknown_schema_are_refused() {
    for xml in [
        r#"<!DOCTYPE pdnImage [<!ENTITY file SYSTEM "file:///private/secret">]><pdnImage width="1" height="1" layers="1">&file;</pdnImage>"#,
        r#"<!DOCTYPE pdnImage SYSTEM "https://example.invalid/schema"><pdnImage width="1" height="1" layers="1"/>"#,
        r#"<?target href="https://example.invalid"?><pdnImage width="1" height="1" layers="1"/>"#,
        r#"<?xml version="1.1"?><pdnImage width="1" height="1" layers="1"/>"#,
        r#"<?xml version="1.0" encoding="UTF-16"?><pdnImage width="1" height="1" layers="1"/>"#,
        r#"<pdnImage xmlns="urn:unknown" width="1" height="1" layers="1"/>"#,
        r#"<unknown width="1" height="1" layers="1"/>"#,
    ] {
        assert_eq!(
            read(&file(xml, &nrbf())).unwrap_err().code,
            ErrorCode::CapabilityUnsupported,
            "{xml}"
        );
    }
}
#[test]
fn source_xml_structure_and_geometry_have_injected_limits() {
    let bytes = file(XML, &nrbf());
    let configurations = [
        Limits {
            source_bytes: bytes.len() - 1,
            ..Default::default()
        },
        Limits {
            header_bytes: XML.len() - 1,
            ..Default::default()
        },
        Limits {
            xml_depth: 1,
            ..Default::default()
        },
        Limits {
            xml_elements: 1,
            ..Default::default()
        },
        Limits {
            attributes_per_element: 3,
            ..Default::default()
        },
        Limits {
            attribute_bytes: 3,
            ..Default::default()
        },
        Limits {
            dimension: 2,
            ..Default::default()
        },
        Limits {
            pixels: 5,
            ..Default::default()
        },
        Limits {
            layers: 0,
            ..Default::default()
        },
    ];
    for limits in configurations {
        assert_eq!(
            inspect(&bytes, &limits, &|| Ok(())).unwrap_err().code,
            ErrorCode::ResourceLimit,
            "{limits:?}"
        );
    }
    let bytes = b"PDN3\xff\xff\xff";
    assert_eq!(read(bytes).unwrap_err().code, ErrorCode::ResourceLimit);
}
#[test]
fn cancellation_preserves_canonical_error_at_multiple_checkpoints() {
    let bytes = file(XML, &nrbf());
    for after in [0, 1, 3, 8] {
        let calls = Cell::new(0);
        let check = || {
            let n = calls.get();
            calls.set(n + 1);
            if n >= after {
                Err(Error::new(
                    ErrorCode::OperationCancelled,
                    "test cancellation",
                ))
            } else {
                Ok(())
            }
        };
        assert_eq!(
            inspect(&bytes, &Limits::default(), &check)
                .unwrap_err()
                .code,
            ErrorCode::OperationCancelled
        );
    }
}
#[test]
fn gzip_is_only_framing_inspection_not_decompression_or_pixel_support() {
    // RFC1952 empty-member bytes. No decompress or CRC implementation is called.
    let gzip = [
        0x1f, 0x8b, 8, 0, 0, 0, 0, 0, 0, 3, 3, 0, 0, 0, 0, 0, 0, 0, 0, 0,
    ];
    let bytes = file(XML, &gzip);
    assert_eq!(
        read(&bytes).unwrap().body_header,
        BodyHeader::Gzip {
            flags: 0,
            header_bytes: 10
        }
    );
    for end in 0..19 {
        assert!(read(&file(XML, &gzip[..end])).is_err());
    }
    let mut wrong = gzip;
    wrong[3] = 0xe0;
    assert!(read(&file(XML, &wrong)).is_err());
    let mut name = gzip.to_vec();
    name[3] = 8;
    name.splice(10..10, b"filename\0".iter().copied());
    assert_eq!(
        read(&file(XML, &name)).unwrap().body_header,
        BodyHeader::Gzip {
            flags: 8,
            header_bytes: 19
        }
    );
    let mut marker = nrbf();
    marker[1] = 2;
    assert_eq!(
        read(&file(XML, &marker)).unwrap_err().code,
        ErrorCode::CapabilityUnsupported
    );
}
