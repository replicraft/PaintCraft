//! PCND3/4 point registry and PCND5 spatial registry; source/evaluated pixels are separate.
use super::*;
use paintcraft_core::types::filters::{
    BlackWhiteV1, BoxBlurV1, BrightnessContrastV1, Descriptor, PosterizeRgbV1, ThresholdLumaV1,
};
use paintcraft_document::{FilterInstance, FilterStack, filters::FILTERS_MAX};

pub(super) fn write<W: Write>(
    stack: &FilterStack,
    output: &mut Output<W>,
    version: u8,
    check: &dyn Fn() -> Result<()>,
    mask_check: &dyn Fn() -> Result<()>,
) -> Result<()> {
    output.number(stack.len() as u64)?;
    for instance in stack.iter() {
        check()?;
        output.bytes(instance.id().as_bytes())?;
        output.bytes(&[u8::from(instance.enabled())])?;
        match instance.descriptor() {
            Descriptor::InvertRgbV1 => output.bytes(&[1])?,
            Descriptor::BrightnessContrastV1(parameters) => {
                output.bytes(&[2])?;
                output.signed(i64::from(parameters.brightness()))?;
                output.signed(i64::from(parameters.contrast()))?;
            }
            Descriptor::BlackWhiteRgbV1(parameters) => {
                output.bytes(&[3, parameters.strength()])?;
            }
            Descriptor::ThresholdLumaV1(parameters) => {
                output.bytes(&[4, parameters.level()])?;
            }
            Descriptor::PosterizeRgbV1(parameters) => {
                output.bytes(&[5, parameters.levels()])?;
            }
            Descriptor::SolarizeRgbV1 => output.bytes(&[6])?,
            Descriptor::BoxBlurV1(parameters) if version == 5 => {
                output.bytes(&[7, parameters.radius()])?
            }
            Descriptor::BoxBlurV1(_) => return Err(unsupported("Spatial filters require PCND5")),
        }
        if version >= 4 {
            masks::write(instance.mask(), output, mask_check)?;
        }
    }
    Ok(())
}

pub(super) fn read(
    input: &mut Input<'_>,
    allowance: usize,
    version: u8,
    mut coverage: Option<&mut super::super::coverage::CoverageReader<'_>>,
    check: &dyn Fn() -> Result<()>,
) -> Result<FilterStack> {
    check()?;
    let start = input.remaining();
    let count = input.number(u64::MAX)?;
    if count > FILTERS_MAX as u64 {
        return Err(Error::new(
            ErrorCode::ResourceLimit,
            "PCND layer filter count exceeds 256",
        ));
    }
    let minimum = count as usize * 18 + start - input.remaining();
    if minimum > allowance {
        return Err(Error::new(
            ErrorCode::ResourceLimit,
            "PCND filter metadata allowance",
        ));
    }
    let mut instances = Vec::new();
    instances.try_reserve_exact(count as usize).map_err(|_| {
        Error::new(
            ErrorCode::ResourceLimit,
            "Cannot allocate PCND filter stack",
        )
    })?;
    for _ in 0..count {
        check()?;
        let id = input.id()?;
        if instances
            .iter()
            .any(|prior: &FilterInstance| prior.id() == id)
        {
            return Err(codec("duplicate layer filter ID"));
        }
        let enabled = match input.byte()? {
            0 => false,
            1 => true,
            _ => return Err(codec("invalid filter enabled flag")),
        };
        let descriptor = match input.byte()? {
            1 => Descriptor::InvertRgbV1,
            2 => {
                let brightness = i16::try_from(input.signed()?)
                    .map_err(|_| codec("invalid filter brightness"))?;
                let contrast =
                    i16::try_from(input.signed()?).map_err(|_| codec("invalid filter contrast"))?;
                Descriptor::BrightnessContrastV1(
                    BrightnessContrastV1::new(brightness, contrast)
                        .map_err(|_| codec("invalid brightness/contrast range"))?,
                )
            }
            3 => Descriptor::BlackWhiteRgbV1(
                BlackWhiteV1::new(i16::from(input.byte()?))
                    .map_err(|_| codec("invalid black and white strength"))?,
            ),
            4 => Descriptor::ThresholdLumaV1(
                ThresholdLumaV1::new(i16::from(input.byte()?))
                    .map_err(|_| codec("invalid luma threshold"))?,
            ),
            5 => Descriptor::PosterizeRgbV1(
                PosterizeRgbV1::new(i16::from(input.byte()?))
                    .map_err(|_| codec("invalid posterize level count"))?,
            ),
            6 => Descriptor::SolarizeRgbV1,
            7 if version == 5 => Descriptor::BoxBlurV1(
                BoxBlurV1::new(i16::from(input.byte()?))
                    .map_err(|_| codec("invalid box radius"))?,
            ),
            _ => return Err(unsupported("unknown PCND filter algorithm/version")),
        };
        if start - input.remaining() > allowance {
            return Err(Error::new(
                ErrorCode::ResourceLimit,
                "PCND filter metadata allowance",
            ));
        }
        let mask = if let Some(reader) = coverage.as_deref_mut() {
            let remaining = allowance
                .checked_sub(start - input.remaining())
                .ok_or_else(|| {
                    Error::new(ErrorCode::ResourceLimit, "PCND filter metadata allowance")
                })?;
            masks::read(input, remaining, reader, check)?
        } else {
            None
        };
        instances.push(FilterInstance::new(id, enabled, descriptor).with_mask(mask));
    }
    FilterStack::new(instances)
}

fn integer_bytes(mut value: u64) -> u64 {
    let mut count = 1;
    while value >= 128 {
        count += 1;
        value >>= 7;
    }
    count
}
fn signed_bytes(value: i16) -> u64 {
    let value = i64::from(value);
    integer_bytes(((value << 1) ^ (value >> 63)) as u64)
}
pub(super) fn metadata_bytes(stack: &FilterStack) -> u64 {
    integer_bytes(stack.len() as u64)
        + stack
            .iter()
            .map(|instance| {
                18 + match instance.descriptor() {
                    Descriptor::InvertRgbV1 => 0,
                    Descriptor::BrightnessContrastV1(parameters) => {
                        signed_bytes(parameters.brightness()) + signed_bytes(parameters.contrast())
                    }
                    Descriptor::BlackWhiteRgbV1(_)
                    | Descriptor::ThresholdLumaV1(_)
                    | Descriptor::PosterizeRgbV1(_)
                    | Descriptor::BoxBlurV1(_) => 1,
                    Descriptor::SolarizeRgbV1 => 0,
                }
            })
            .sum::<u64>()
}
