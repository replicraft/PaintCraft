use super::*;
use paintcraft_core::types::{
    ErrorCode,
    filters::{AddNoiseRgbV1, NoiseDistribution},
};
#[test]
fn noise_pcnd6_parameters_are_exact_six_bytes_including_unsigned_seed_and_flags() {
    require("add-noise-rgb-v1", "params-v1").unwrap();
    for amount in [0, 10, 100] {
        for seed in [0, 1, u16::MAX] {
            for mono in [false, true] {
                for distribution in [NoiseDistribution::Uniform, NoiseDistribution::Gaussian] {
                    let p = AddNoiseRgbV1::new(amount, seed, mono, distribution).unwrap();
                    let d = Descriptor::AddNoiseRgbV1(p);
                    let (bytes, len) = encode(d);
                    assert_eq!(len, 6);
                    assert_eq!(bytes, p.semantic_bytes());
                    assert_eq!(decode("add-noise-rgb-v1", &bytes).unwrap(), d);
                }
            }
        }
    }
}
#[test]
fn noise_pcnd_flags_lengths_and_ranges_refuse_without_defaulting() {
    for bytes in [
        &[][..],
        &[0; 5][..],
        &[0; 7][..],
        &[0, 101, 0, 0, 0, 0][..],
        &[255, 255, 0, 0, 0, 0][..],
        &[0, 0, 255, 255, 2, 0][..],
        &[0, 0, 255, 255, 0, 2][..],
    ] {
        assert_eq!(
            decode("add-noise-rgb-v1", bytes).unwrap_err().code,
            ErrorCode::CodecFailed
        );
    }
    assert_eq!(
        require("add-noise-rgb-v1", "params-v2").unwrap_err().code,
        ErrorCode::CapabilityUnsupported
    );
    assert_eq!(
        require("unknown-noise-v1", "params-v1").unwrap_err().code,
        ErrorCode::CapabilityUnsupported
    );
}
