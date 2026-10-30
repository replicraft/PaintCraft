//! PCND3 point-filter intentions; no source raster or evaluated pixels are encoded here.
use super::*;
use paintcraft_core::types::filters::{BrightnessContrastV1, Descriptor};
use paintcraft_document::{FilterInstance, FilterStack, filters::FILTERS_MAX};

pub(super) fn write<W: Write>(
    stack: &FilterStack,
    output: &mut Output<W>,
    masked: bool,
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
        }
        if masked {
            masks::write(instance.mask(), output, mask_check)?;
        }
    }
    Ok(())
}

pub(super) fn read(
    input: &mut Input<'_>,
    allowance: usize,
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
                }
            })
            .sum::<u64>()
}
