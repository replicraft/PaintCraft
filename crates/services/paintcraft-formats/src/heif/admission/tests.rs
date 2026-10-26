use super::*;
const PRIMARY: &[u8] = include_bytes!("../../../tests/heif-fixtures/primary.heic");
const GRID: &[u8] = include_bytes!("../../../tests/heif-fixtures/grid.heic");
const ALPHA: &[u8] = include_bytes!("../../../tests/heif-fixtures/alpha.heic");
#[test]
fn every_coded_sps_is_admitted_with_exact_native_dimensions_before_frame_decode() {
    for (bytes, size, alpha) in [
        (PRIMARY, (32, 24), false),
        (GRID, (64, 24), false),
        (ALPHA, (32, 24), true),
    ] {
        let admitted = admit_gray8(bytes, PIXELS_MAX, &|| Ok(())).unwrap();
        assert_eq!(admitted.size(), size);
        assert_eq!(admitted.alpha(), alpha);
        assert_eq!(admitted.source().as_ptr(), bytes.as_ptr());
    }
    assert_eq!(
        admit_gray8(PRIMARY, 767, &|| Ok(())).unwrap_err().code,
        ErrorCode::ResourceLimit
    );
    assert_eq!(
        admit_gray8(PRIMARY, PIXELS_MAX, &|| Err(Error::new(
            ErrorCode::OperationCancelled,
            "cancel"
        )))
        .unwrap_err()
        .code,
        ErrorCode::OperationCancelled
    );
}
#[test]
fn precision_ispe_cropped_geometry_and_array_nal_type_must_match_actual_sps() {
    let value = inspect_checked(PRIMARY, PIXELS_MAX, &|| Ok(())).unwrap();
    let original = value.item(7).unwrap();
    for property in [
        Property::PixelInformation { bits: &[10] },
        Property::SpatialExtent {
            width: 31,
            height: 24,
        },
    ] {
        let mut item = original.clone();
        let index = if matches!(property, Property::PixelInformation { .. }) {
            2
        } else {
            0
        };
        item.properties[index].property = property;
        assert!(coded(&item, PIXELS_MAX, &|| Ok(())).is_err());
    }
    let config = original
        .properties
        .iter()
        .find_map(|a| {
            if let Property::HevcConfig { bytes, .. } = a.property {
                Some(bytes)
            } else {
                None
            }
        })
        .unwrap();
    let mut wrong = config.to_vec();
    // First array claims VPS but its actual NAL header is SPS.
    wrong[28] = (33 << 1) | (wrong[28] & 1);
    let mut item = original.clone();
    let Property::HevcConfig { bytes, .. } = &mut item.properties[1].property else {
        panic!()
    };
    *bytes = &wrong;
    assert!(coded(&item, PIXELS_MAX, &|| Ok(())).is_err());
}
#[test]
fn native_encoding_never_guesses_icc_limited_range_hdr_or_transform_semantics() {
    let value = inspect_checked(PRIMARY, PIXELS_MAX, &|| Ok(())).unwrap();
    let encoding = Encoding {
        primaries: 2,
        transfer: 2,
        matrix: 2,
        full_range: true,
    };
    for property in [
        Property::Color(Color::Icc {
            restricted: false,
            bytes: &[0; 16],
        }),
        Property::Rotation {
            quarter_turns_ccw: 1,
        },
        Property::Mirror { axis: 0 },
        Property::Color(Color::Nclx {
            primaries: 2,
            transfer: 2,
            matrix: 2,
            full_range: false,
        }),
        Property::Color(Color::Nclx {
            primaries: 9,
            transfer: 16,
            matrix: 9,
            full_range: true,
        }),
    ] {
        let mut item = value.item(7).unwrap().clone();
        item.properties.push(Association {
            index: 5,
            essential: true,
            property,
        });
        assert!(profile(&item, Some(encoding)).is_err());
    }
    assert!(profile(value.item(7).unwrap(), None).is_err());
}
