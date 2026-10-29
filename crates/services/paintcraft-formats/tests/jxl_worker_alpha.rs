//! Independent libjxl source metadata, never inferred from output pixels.
use ::jxl::api::{Event, JxlBitDepth, JxlDecoder, JxlDecoderOptions};
use paintcraft_core::types::ErrorCode;
use paintcraft_formats::jxl;
use sha2::{Digest, Sha256};

#[test]
fn real_mixed_float_and_named_alpha_sources_are_refused_before_worker_pixels() {
    for (source, sha, color, alpha, named) in [
        (
            include_bytes!("jxl-fixtures/alpha-profiles/color8-alpha16.jxl").as_slice(),
            "10cd17935737884bf58ff2de740160d63cd354077c04de45e4a484ed0150840f",
            8,
            JxlBitDepth::Int {
                bits_per_sample: 16,
            },
            false,
        ),
        (
            include_bytes!("jxl-fixtures/alpha-profiles/color16-alpha8.jxl"),
            "df42b6b59b48cb404ca08645f9f65e3436b83e76e20109ea9ae08b5665d3076a",
            16,
            JxlBitDepth::Int { bits_per_sample: 8 },
            false,
        ),
        (
            include_bytes!("jxl-fixtures/alpha-profiles/alpha-float.jxl"),
            "14823dee0b04f708c68056dc49363eb98cee09c169259bfacfcd684cb4c0eaad",
            16,
            JxlBitDepth::Float {
                bits_per_sample: 16,
                exponent_bits_per_sample: 5,
            },
            false,
        ),
        (
            include_bytes!("jxl-fixtures/alpha-profiles/alpha-named.jxl"),
            "dea185f8e2c53293744ac73e2bcb462ee93d3ae2d433a1aba64a948cc4e25575",
            16,
            JxlBitDepth::Int {
                bits_per_sample: 16,
            },
            true,
        ),
    ] {
        assert_eq!(paintcraft_document::hex(&Sha256::digest(source)), sha);
        let mut decoder = JxlDecoder::new(JxlDecoderOptions::default());
        decoder.close_input();
        let mut input = source;
        assert_eq!(
            decoder.process(&mut input, None, None).unwrap(),
            Event::BasicInfo
        );
        let info = decoder.basic_info().unwrap();
        assert_eq!(info.size, (2, 2));
        assert_eq!(
            info.bit_depth,
            JxlBitDepth::Int {
                bits_per_sample: color
            }
        );
        assert_eq!(info.extra_channels.len(), 1);
        let actual = &info.extra_channels[0];
        assert_eq!(actual.bit_depth, alpha);
        assert_eq!(actual.dim_shift, 0);
        assert_eq!(actual.name_present, named);
        assert!(!actual.alpha_associated);

        let error = jxl::decode_worker_integer_checked(source, 4, &|| Ok(())).unwrap_err();
        assert_eq!(error.code, ErrorCode::CapabilityUnsupported);
        assert!(error.message.contains("alpha must"), "{}", error.message);
        assert!(jxl::decode_worker_rgba8_checked(source, 4, &|| Ok(())).is_err());
        assert!(jxl::decode_worker_rgba16_checked(source, 4, &|| Ok(())).is_err());
    }
}
