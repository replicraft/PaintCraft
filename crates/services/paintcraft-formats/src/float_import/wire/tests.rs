use super::*;
#[test]
fn finite_settings_roundtrip_and_every_reserved_unknown_or_noncanonical_option_refuses() {
    for curve in [Curve::Clip, Curve::ReinhardPerChannel] {
        for negative in [Negative::Reject, Negative::ClipToBlack] {
            for alpha_output in [
                AlphaOutput::Preserve,
                AlphaOutput::Flatten {
                    srgb_background: [1, 127, 255],
                },
            ] {
                for source_assignment in [
                    None,
                    Some(LinearSpace::Rec709D65),
                    Some(LinearSpace::RadianceEqualEnergy),
                ] {
                    for alpha_assignment in [
                        None,
                        Some(AlphaAssignment::Premultiplied),
                        Some(AlphaAssignment::Straight),
                    ] {
                        for exposure_milli_ev in [-16000, 0, 16000] {
                            let settings = Settings {
                                curve,
                                negative,
                                alpha_output,
                                source_assignment,
                                alpha_assignment,
                                exposure_milli_ev,
                            };
                            let encoded = encode_settings(settings).unwrap();
                            assert_eq!(decode_settings(&encoded).unwrap(), settings);
                        }
                    }
                }
            }
        }
    }
    let bytes = encode_settings(Settings::default()).unwrap();
    for at in [0, 8, 9, 10, 11, 12, 13, 14, 15, 20, 21, 22]
        .into_iter()
        .chain(23..32)
    {
        let mut invalid = bytes;
        invalid[at] = 255;
        assert!(decode_settings(&invalid).is_err(), "offset{at}");
    }
    for n in 0..32 {
        assert!(decode_settings(&bytes[..n]).is_err());
    }
    let mut extra = bytes.to_vec();
    extra.push(0);
    assert!(decode_settings(&extra).is_err());
    assert!(
        encode_settings(Settings {
            exposure_milli_ev: 16001,
            ..Settings::default()
        })
        .is_err()
    );
}
#[test]
fn fixed_report_offsets_are_distinct_and_reserved_or_truncated_reports_refuse() {
    let report = Report {
        digest: [99; 32],
        statistics: Statistics {
            negative_source_samples: 1,
            negative_target_samples_clipped: 2,
            highlights_clipped: 3,
            additive_zero_alpha_pixels: 4,
            alpha_samples_quantized: 5,
        },
        partial: 6,
    };
    let bytes = report.encode();
    assert_eq!(bytes.len(), 96);
    assert_eq!(Report::decode(&bytes).unwrap(), report);
    assert_eq!(&bytes[16..48], &[99; 32]);
    for (index, count) in (1u64..=6).enumerate() {
        assert_eq!(&bytes[48 + 8 * index..56 + 8 * index], &count.to_be_bytes());
    }
    for at in [0, 8, 9, 10, 11, 12, 13, 14, 15] {
        let mut invalid = bytes;
        invalid[at] = 255;
        assert!(Report::decode(&invalid).is_err());
    }
    for n in 0..96 {
        assert!(Report::decode(&bytes[..n]).is_err());
    }
}
