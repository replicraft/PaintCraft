use super::*;

#[test]
fn authenticated_preview_profiles_refuse_future_versions_at_the_consumer_boundary() {
    for protocol in [2, 3] {
        let doc = if protocol == 2 {
            fixture()
        } else {
            spatial_source(1, 1, 0, false)
        };
        let original = pcnd::write(&doc).unwrap();
        let control = |bytes: &[u8]| {
            if protocol == 2 {
                plan(&doc, bytes)
            } else {
                profile(&doc, bytes)
            }
        };
        assert!(Service::open(&original, &text(&control(&original))).is_ok());
        for version in [0, 6, 255] {
            let mut bytes = original.clone();
            bytes[4] = version;
            let end = bytes.len() - 32;
            let outer = Sha256::digest(&bytes[..end]);
            bytes[end..].copy_from_slice(&outer);
            let value = control(&bytes);
            let error = Service::open(&bytes, &text(&value)).unwrap_err();
            assert_eq!(error.code, ErrorCode::CapabilityUnsupported);
            assert_eq!(
                error.message,
                if protocol == 2 {
                    "Preview source2 admits only PCND1 through PCND4"
                } else {
                    "Preview source3 admits only PCND1 through PCND5"
                }
            );
            bytes[4] ^= 1;
            let error = Service::open(&bytes, &text(&value)).unwrap_err();
            assert_eq!(error.code, ErrorCode::ConflictStaleBase);
        }
    }
}

#[test]
fn preview_profiles_preserve_their_supported_source_versions() {
    let sources = [
        fixture(),
        retained_source(false),
        pcnd::read(include_bytes!(concat!(env!("CARGO_MANIFEST_DIR"), "/../../../../crates/services/paintcraft-formats/tests/browser-filter-export-fixtures/invert.pcnd"))).unwrap(),
        masked_plane(),
        spatial_source(1, 1, 0, false),
    ];
    for (index, doc) in sources.iter().enumerate() {
        let bytes = pcnd::write(doc).unwrap();
        assert_eq!(usize::from(bytes[4]), index + 1);
        assert!(Service::open(&bytes, &text(&profile(doc, &bytes))).is_ok());
        let legacy = Service::open(&bytes, &text(&plan(doc, &bytes)));
        if index < 4 {
            assert!(legacy.is_ok());
        } else {
            let error = legacy.unwrap_err();
            assert_eq!(error.code, ErrorCode::CapabilityUnsupported);
            assert_eq!(error.message, protocol::spatial::refused().message);
        }
    }
}
