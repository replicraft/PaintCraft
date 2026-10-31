use super::*;
use paintcraft_core::types::filters::CurvesRgbV1;
#[test]
fn actual_curves_worker7_causal_pixels_disabled_intent_and_source_immutability() {
    for codec in [Codec::Dds, Codec::Msp] {
        for enabled in [false, true] {
            let mut doc = source(codec, enabled, 0);
            if !enabled && matches!(codec, Codec::Msp) {
                doc = paintcraft_document::import_image(1, 1, &[255; 4], "disabled Curves white")
                    .unwrap();
            }
            let points = match codec {
                Codec::Dds => [255, 191, 127, 63, 0],
                Codec::Msp => [0; 5],
            };
            doc.layers.get_mut(&doc.roots[0]).unwrap().filters =
                FilterStack::new(vec![FilterInstance::new(
                    Uuid::from_u128(17),
                    enabled,
                    Descriptor::CurvesRgbV1(CurvesRgbV1::new(points)),
                )])
                .unwrap();
            let original = pcnd::write(&doc).unwrap();
            let request = packet(&doc, codec);
            let Outcome::Success(encoded, preview, losses) =
                execute(&request, codec, &|| Ok(())).unwrap()
            else {
                panic!("valid Curves refused")
            };
            let expected = match (codec, enabled) {
                (Codec::Dds, true) => [191, 239, 127, 255],
                (Codec::Dds, false) => [64, 16, 128, 255],
                (Codec::Msp, true) => [0, 0, 0, 255],
                (Codec::Msp, false) => [255; 4],
            };
            assert_eq!(preview, expected);
            let decoded = match codec {
                Codec::Dds => {
                    dds::decode_top_mip_checked(&encoded, 1, &|| Ok(()))
                        .unwrap()
                        .rgba
                }
                Codec::Msp => paintcraft_formats::msp::decode(&encoded, &|| Ok(()))
                    .unwrap()
                    .rgba(&|| Ok(()))
                    .unwrap(),
            };
            assert_eq!(decoded, expected);
            assert!(losses.iter().any(|s| s == filter_bake::BAKED_REASON));
            assert!(!losses.iter().any(|s| s == metadata_policy::OMITTED_REASON));
            assert_eq!(pcnd::write(&doc).unwrap(), original);
        }
    }
}
#[test]
fn curves_prepared_bake_authority_refuses_before_output_and_old_protocol_stays_closed() {
    for codec in [Codec::Dds, Codec::Msp] {
        let mut doc = source(codec, false, 0);
        doc.layers.get_mut(&doc.roots[0]).unwrap().filters =
            FilterStack::new(vec![FilterInstance::new(
                Uuid::from_u128(17),
                false,
                Descriptor::CurvesRgbV1(CurvesRgbV1::default()),
            )])
            .unwrap();
        let request = packet(&doc, codec);
        let mut revoked = request.clone();
        // Bake intent is part of the authenticated header; altering it cannot mint authority.
        revoked[122] = 0;
        assert_eq!(
            error(execute(&revoked, codec, &|| Ok(()))).code,
            ErrorCode::CapabilityUnsupported
        );
        assert_eq!(
            error(execute(&request, codec, &|| Err(Error::new(
                ErrorCode::OperationCancelled,
                "cancel Curves"
            ))))
            .code,
            ErrorCode::OperationCancelled
        );
    }
}
