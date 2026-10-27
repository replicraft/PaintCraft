use super::*;
fn options() -> TextureOptions {
    TextureOptions {
        encoding: Encoding::Rgba8,
        transfer: Transfer::Srgb,
        alpha: SourceAlpha::Straight,
        mips: MipCount::Levels(1),
        filter: MipFilter::Nearest,
        quality: TextureQuality::Normal,
    }
}
fn doc(width: u32, height: u32) -> Document {
    paintcraft_document::new_document(
        width,
        height,
        Some(paintcraft_core::types::Rgba([255, 0, 0, 255])),
        0,
        paintcraft_document::POLICY,
    )
    .unwrap()
}
fn capture(doc: &Document, options: TextureOptions) -> (Header, Vec<u8>) {
    let plan = SourcePlan::capture(doc, options).unwrap();
    assert_eq!(plan.source_flags() & 1, u8::from(doc.roots.len() > 1));
    let source = plan.snapshot(doc).unwrap();
    let header = Header::capture(
        &plan,
        &source,
        Owner {
            session: Uuid::new_v4(),
            state: doc.state_id,
            epoch: 7,
            frontend: 3,
        },
        Uuid::new_v4(),
    )
    .unwrap();
    let mut bytes = header.encode().to_vec();
    bytes.extend(source);
    (header, bytes)
}
fn success(bytes: &[u8], header: &Header) -> (Vec<u8>, Vec<u8>) {
    match decode_response(bytes, header, header.owner, "0.1.0").unwrap() {
        Frame::Success {
            encoded, preview, ..
        } => (encoded.to_vec(), preview.to_vec()),
        Frame::Refused(error) => panic!("{error:?}"),
    }
}
#[test]
fn exact_golden_pipeline_and_default_bytes_use_one_complete_snapshot() {
    let pixels =
        include_bytes!("../../../tests/dds-texture-write-fixtures/sources/7x5-opaque.rgba");
    let mut document = doc(7, 5);
    document.layers.get_mut(&document.roots[0]).unwrap().raster =
        paintcraft_raster::RasterLayer::from_rgba(
            7,
            5,
            paintcraft_core::types::Rgba([0, 0, 0, 255]),
            pixels,
            &mut paintcraft_raster::Admission::new(1 << 20),
        )
        .unwrap();
    let original = pcnd::write(&document).unwrap();
    let selected = TextureOptions {
        encoding: Encoding::Bc1,
        alpha: SourceAlpha::Opaque,
        mips: MipCount::Full,
        filter: MipFilter::BoxStored,
        ..options()
    };
    let (header, bytes) = capture(&document, selected);
    assert_eq!(&bytes[REQUEST_HEADER..], original);
    let output = execute(&bytes, "0.1.0", &|| Ok(())).unwrap();
    let (encoded, preview) = success(&output, &header);
    assert_eq!(
        encoded,
        include_bytes!(
            "../../../tests/dds-texture-write-fixtures/normal/7x5-Bc1-Srgb-BoxStored.dds"
        )
    );
    assert_ne!(
        encoded,
        include_bytes!("../../../tests/dds-texture-write-fixtures/normal/7x5-Bc1-Srgb-Nearest.dds")
    );
    let actual = decode_top_mip_checked(&encoded, 35, &|| Ok(())).unwrap();
    assert_eq!(preview, actual.rgba);
    assert_eq!(pcnd::write(&document).unwrap(), original);
    let (header, bytes) = capture(&document, options());
    let output = execute(&bytes, "0.1.0", &|| Ok(())).unwrap();
    let (encoded, preview) = success(&output, &header);
    assert_eq!(encoded, encode_rgba8(7, 5, pixels, Transfer::Srgb).unwrap());
    assert_eq!(preview, pixels);
}
#[test]
fn settings_and_request_layout_are_canonical_with_nonoverlapping_reserved_fields() {
    let (header, bytes) = capture(&doc(7, 5), options());
    assert_eq!(Header::decode(&bytes[..REQUEST_HEADER]).unwrap(), header);
    for index in [0, 8, 10, 11, 40, 113, 119, 120, 122, 127] {
        let mut changed = bytes[..REQUEST_HEADER].to_vec();
        changed[index] ^= 0xff;
        // Owner UUID bytes are arbitrary until the captured response comparison.
        if index == 40 {
            assert_ne!(Header::decode(&changed).unwrap(), header);
        } else {
            assert!(Header::decode(&changed).is_err(), "{index}");
        }
    }
    let mut changed = header.clone();
    changed.owner.frontend = 2;
    assert!(Header::decode(&changed.encode()).is_err());
    let mut changed = header.clone();
    changed.source_len = SOURCE_BYTES_MAX as u32 + 1;
    assert!(Header::decode(&changed.encode()).is_err());
    let mut changed = header.clone();
    changed.size = (1_048_577, 1);
    assert!(Header::decode(&changed.encode()).is_err());
    let mut bad = header.settings;
    bad[4] = 1;
    assert!(
        decode_settings(bad, header.size).is_err(),
        "inactive filter is not canonical"
    );
    let mut bad = header.settings;
    bad[5] = 1;
    assert!(
        decode_settings(bad, header.size).is_err(),
        "inactive RGBA effort is not canonical"
    );
    let mut bad = header.settings;
    bad[2] = 1;
    assert!(decode_settings(bad, header.size).is_err());
    assert!(Header::decode(&bytes[..REQUEST_HEADER - 1]).is_err());
}
#[test]
fn hostile_response_hash_owner_options_version_geometry_counts_and_lengths_fail_closed() {
    let (header, bytes) = capture(&doc(7, 5), options());
    let response = execute(&bytes, "0.1.0", &|| Ok(())).unwrap();
    for index in [
        0,
        8,
        11,
        12,
        20,
        24,
        28,
        32,
        73,
        89,
        121,
        129,
        161,
        193,
        194,
        198,
        199,
        263,
        265,
        266,
        287,
        RESPONSE_HEADER,
    ] {
        let mut altered = response.clone();
        altered[index] ^= 0xff;
        assert!(
            decode_response(&altered, &header, header.owner, "0.1.0").is_err(),
            "{index}"
        );
    }
    let mut owner = header.owner;
    owner.epoch += 1;
    assert!(decode_response(&response, &header, owner, "0.1.0").is_err());
    assert!(decode_response(&response, &header, header.owner, "0.1.1").is_err());
    assert!(
        decode_response(
            &response[..response.len() - 1],
            &header,
            header.owner,
            "0.1.0"
        )
        .is_err()
    );
    let mut extended = response.clone();
    extended.push(0);
    assert!(decode_response(&extended, &header, header.owner, "0.1.0").is_err());
}
#[test]
fn canonical_refusals_preserve_binding_and_source_interpretation_without_output() {
    let (header, mut bytes) = capture(&doc(7, 5), options());
    bytes[REQUEST_HEADER] ^= 1;
    let output = execute(&bytes, "0.1.0", &|| Ok(())).unwrap();
    assert!(matches!(
        decode_response(&output, &header, header.owner, "0.1.0").unwrap(),
        Frame::Refused(Error {
            code: ErrorCode::CodecFailed,
            ..
        })
    ));
    let mut changed = output.clone();
    changed[193] = 1;
    assert!(decode_response(&changed, &header, header.owner, "0.1.0").is_err());
    let (mut header, mut bytes) = capture(&doc(7, 5), options());
    header.source_flags = 1;
    bytes[..REQUEST_HEADER].copy_from_slice(&header.encode());
    let output = execute(&bytes, "0.1.0", &|| Ok(())).unwrap();
    assert!(matches!(
        decode_response(&output, &header, header.owner, "0.1.0").unwrap(),
        Frame::Refused(_)
    ));
}
#[test]
fn document_profile_refuses_instead_of_dropping_layers_or_excess_work() {
    assert!(SourcePlan::capture(&doc(1025, 1024), options()).is_err());
    assert!(SourcePlan::capture(&doc(65537, 1), options()).is_err());
    let mut document = doc(1, 1);
    for _ in 0..8 {
        let mut layer = document.layers[&document.roots[0]].clone();
        layer.id = Uuid::new_v4();
        document.roots.push(layer.id);
        document.layers.insert(layer.id, layer);
    }
    assert!(SourcePlan::capture(&document, options()).is_err());
    let mut document = doc(1, 1);
    document.layers.get_mut(&document.roots[0]).unwrap().raster =
        paintcraft_raster::RasterLayer::new(
            2048,
            2048,
            paintcraft_core::types::Rgba([0, 0, 0, 255]),
        );
    assert!(SourcePlan::capture(&document, options()).is_err());
}
#[test]
fn actual_one_megapixel_profile_boundary_retains_all_generated_levels_and_preview() {
    let document = doc(1024, 1024);
    let selected = TextureOptions {
        mips: MipCount::Full,
        filter: MipFilter::BoxStored,
        ..options()
    };
    let (header, bytes) = capture(&document, selected);
    let output = execute(&bytes, "0.1.0", &|| Ok(())).unwrap();
    let (encoded, preview) = success(&output, &header);
    let inventory = inspect_top_mip_checked(&encoded, PIXELS_MAX, &|| Ok(())).unwrap();
    assert_eq!(inventory.levels.len(), 11);
    assert_eq!(preview.len(), 192 * 192 * 4);
    assert!(
        preview
            .as_chunks::<4>()
            .0
            .iter()
            .all(|p| *p == [255, 0, 0, 255])
    );
    assert!(encoded.len() <= ENCODED_BYTES_MAX);
}
#[test]
fn linear_preview_is_stored_samples_for_explicit_host_display_conversion() {
    let mut document = doc(1, 1);
    document.layers.get_mut(&document.roots[0]).unwrap().raster =
        paintcraft_raster::RasterLayer::new(
            1,
            1,
            paintcraft_core::types::Rgba([128, 188, 255, 127]),
        );
    let (header, bytes) = capture(
        &document,
        TextureOptions {
            transfer: Transfer::Linear,
            ..options()
        },
    );
    let output = execute(&bytes, "0.1.0", &|| Ok(())).unwrap();
    match decode_response(&output, &header, header.owner, "0.1.0").unwrap() {
        Frame::Success {
            preview,
            losses,
            quantized_rgb_samples,
            ..
        } => {
            assert_eq!(preview, [55, 128, 255, 127]);
            assert_eq!(quantized_rgb_samples, 2);
            assert!(losses.contains(&export::dds::LINEAR_QUANTIZATION_LOSS));
        }
        Frame::Refused(error) => panic!("{error:?}"),
    }
}

#[test]
fn pcnd_host_layer_admission_precedes_records_and_preserves_legacy_wrappers() {
    let base = doc(1, 1);
    let plan = SourcePlan::capture(&base, options()).unwrap();
    let mut document = base.clone();
    for _ in 0..8 {
        let mut layer = document.layers[&document.roots[0]].clone();
        layer.id = Uuid::new_v4();
        document.roots.push(layer.id);
        document.layers.insert(layer.id, layer);
    }
    let source = pcnd::write(&document).unwrap();
    assert_eq!(pcnd::read(&source).unwrap().roots.len(), 9);
    assert_eq!(
        pcnd::read_with_limits_checked(&source, PIXELS_MAX, RESIDENT_BYTES_MAX / 4, &|| Ok(()))
            .unwrap()
            .roots
            .len(),
        9
    );
    let error = pcnd::read_with_layer_limit_checked(
        &source,
        PIXELS_MAX,
        RESIDENT_BYTES_MAX / 4,
        LAYERS_MAX,
        &|| Ok(()),
    )
    .unwrap_err();
    assert_eq!(error.code, ErrorCode::ResourceLimit);
    assert!(error.message.contains("host layer limit"));
    assert!(
        pcnd::read_with_layer_limit_checked(
            &source,
            PIXELS_MAX,
            RESIDENT_BYTES_MAX / 4,
            paintcraft_document::LAYER_COUNT_MAX + 1,
            &|| Ok(())
        )
        .is_err()
    );
    let owner = Owner {
        session: Uuid::new_v4(),
        state: document.state_id,
        epoch: 0,
        frontend: 3,
    };
    let header = Header::capture(&plan, &source, owner, Uuid::new_v4()).unwrap();
    let mut bytes = header.encode().to_vec();
    bytes.extend(source);
    let output = execute(&bytes, "0.1.0", &|| Ok(())).unwrap();
    match decode_response(&output, &header, owner, "0.1.0").unwrap() {
        Frame::Refused(error) => {
            assert_eq!(error.code, ErrorCode::ResourceLimit);
            assert!(error.message.contains("host layer limit"));
        }
        Frame::Success { .. } => panic!("profile must retain all nine layers or refuse"),
    }
}
