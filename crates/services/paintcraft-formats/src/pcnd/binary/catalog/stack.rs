use super::*;
use paintcraft_document::{FilterInstance, FilterStack, filters::FILTERS_MAX};
pub(in super::super) fn write_stack<W: Write>(
    manifest: &Manifest<'_>,
    stack: &FilterStack,
    output: &mut Output<W>,
    check: &dyn Fn() -> Result<()>,
    mask_check: &dyn Fn() -> Result<()>,
) -> Result<()> {
    output.number(stack.len() as u64)?;
    for filter in stack.iter() {
        check()?;
        let descriptor = filter.descriptor();
        let index = manifest.entries[..manifest.len]
            .iter()
            .position(|e| e.unwrap().profile == descriptor.id())
            .ok_or_else(invalid)?;
        let (bytes, len) = params::encode(descriptor);
        output.bytes(filter.id().as_bytes())?;
        output.bytes(&[u8::from(filter.enabled())])?;
        output.number(index as u64)?;
        output.number(len as u64)?;
        output.bytes(&bytes[..len])?;
        masks::write(filter.mask(), output, mask_check)?;
    }
    Ok(())
}
pub(in super::super) fn read_stack(
    manifest: &mut Manifest<'_>,
    input: &mut Input<'_>,
    allowance: usize,
    reader: &mut super::super::super::coverage::CoverageReader<'_>,
    check: &dyn Fn() -> Result<()>,
) -> Result<FilterStack> {
    check()?;
    let start = input.remaining();
    let count = input.number(FILTERS_MAX as u64)? as usize;
    if count * 20 + start - input.remaining() > allowance {
        return Err(Error::new(
            ErrorCode::ResourceLimit,
            "PCND catalog metadata allowance",
        ));
    }
    let mut entries = Vec::new();
    entries
        .try_reserve_exact(count)
        .map_err(|_| Error::new(ErrorCode::ResourceLimit, "PCND catalog stack allocation"))?;
    for _ in 0..count {
        check()?;
        let id = input.id()?;
        if entries.iter().any(|f: &FilterInstance| f.id() == id) {
            return Err(codec("duplicate layer filter ID"));
        }
        let enabled = match input.byte()? {
            0 => false,
            1 => true,
            _ => return Err(invalid()),
        };
        let index = input.number(u64::MAX)?;
        let entry = usize::try_from(index)
            .ok()
            .filter(|n| *n < manifest.len)
            .and_then(|n| manifest.entries[n])
            .ok_or_else(invalid)?;
        let length = input.number(65_536)? as usize;
        let bytes = input.take(length)?;
        let descriptor = params::decode(entry.profile, bytes)?;
        let remaining = allowance
            .checked_sub(start - input.remaining())
            .ok_or_else(|| {
                Error::new(ErrorCode::ResourceLimit, "PCND catalog metadata allowance")
            })?;
        let mask = masks::read(input, remaining, reader, check)?;
        manifest.used[index as usize] = true;
        entries.push(FilterInstance::new(id, enabled, descriptor).with_mask(mask));
    }
    FilterStack::new(entries)
}
/// Indexes fit two bytes under the fixed 256-entry cap. This is a safe bound;
/// exact raw/compact admission uses the authoritative writer near the ceiling.
pub(in super::super) fn stack_bytes(stack: &FilterStack) -> u64 {
    number_bytes(stack.len() as u64)
        + stack
            .iter()
            .map(|f| {
                let (_, len) = params::encode(f.descriptor());
                19 + number_bytes(len as u64) + len as u64
            })
            .sum::<u64>()
}
