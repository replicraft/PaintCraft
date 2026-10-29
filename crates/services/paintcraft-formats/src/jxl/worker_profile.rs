//! Source extra-channel facts must match the strict worker's retained precision.
use super::*;
use ::jxl::api::{ExtraChannel, JxlBitDepth, JxlExtraChannel};

pub(super) fn admit_channels(depth: u32, channels: &[JxlExtraChannel]) -> Result<()> {
    if !matches!(depth, 8 | 16)
        || channels.len() > 1
        || channels.iter().any(|channel| {
            channel.ec_type != ExtraChannel::Alpha
                || channel.alpha_associated
                || channel.bit_depth
                    != (JxlBitDepth::Int {
                        bits_per_sample: depth,
                    })
                || channel.dim_shift != 0
                || channel.name_present
        })
    {
        return Err(unsupported(
            "Isolated JPEG XL alpha must be unnamed straight integer samples at full color depth and dimensions",
        ));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn actual_channel_facts_never_inherit_main_precision_or_dimensions() {
        for depth in [8, 16] {
            let alpha = JxlExtraChannel {
                ec_type: ExtraChannel::Alpha,
                bit_depth: JxlBitDepth::Int {
                    bits_per_sample: depth,
                },
                dim_shift: 0,
                name_present: false,
                alpha_associated: false,
            };
            assert!(admit_channels(depth, &[]).is_ok());
            assert!(admit_channels(depth, std::slice::from_ref(&alpha)).is_ok());
            for fault in 0..7 {
                let mut invalid = alpha.clone();
                match fault {
                    0 => {
                        invalid.bit_depth = JxlBitDepth::Int {
                            bits_per_sample: 24 - depth,
                        }
                    }
                    1 => {
                        invalid.bit_depth = JxlBitDepth::Float {
                            bits_per_sample: 16,
                            exponent_bits_per_sample: 5,
                        }
                    }
                    2 => invalid.dim_shift = 1,
                    3 => invalid.name_present = true,
                    4 => invalid.alpha_associated = true,
                    5 => invalid.ec_type = ExtraChannel::Depth,
                    _ => {
                        invalid.bit_depth = JxlBitDepth::Int {
                            bits_per_sample: 12,
                        }
                    }
                }
                assert_eq!(
                    admit_channels(depth, &[invalid]).unwrap_err().code,
                    ErrorCode::CapabilityUnsupported
                );
            }
            assert!(admit_channels(depth, &[alpha.clone(), alpha]).is_err());
        }
    }
}
