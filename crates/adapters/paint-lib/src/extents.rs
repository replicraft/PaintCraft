use super::*;
use std::ops::Range;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Extent {
    pub offset: u64,
    pub length: u64,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum AliasPolicy {
    Reject,
    AllowExact,
}
fn range(extent: Extent, scope: &Range<u64>) -> Result<Range<u64>> {
    if scope.start > scope.end {
        return Err(malformed("Invalid ISO extent scope"));
    }
    let end = extent
        .offset
        .checked_add(extent.length)
        .ok_or_else(|| malformed("ISO extent offset overflow"))?;
    if extent.offset < scope.start || end > scope.end {
        return Err(malformed("ISO extent exceeds its data scope"));
    }
    Ok(extent.offset..end)
}
/// Resolve an already interpreted, file-relative extent only inside an admitted
/// data payload. External references and iloc base-offset semantics are not
/// inferred here; a future item parser must validate those before calling.
pub fn resolve_extent(bytes: &[u8], scope: Range<u64>, extent: Extent) -> Result<&[u8]> {
    if scope.end > bytes.len() as u64 {
        return Err(malformed("ISO extent scope exceeds input"));
    }
    let range = range(extent, &scope)?;
    let start = usize::try_from(range.start)
        .map_err(|_| resource("ISO extent offset exceeds target address space"))?;
    let end = usize::try_from(range.end)
        .map_err(|_| resource("ISO extent end exceeds target address space"))?;
    bytes
        .get(start..end)
        .ok_or_else(|| malformed("ISO extent exceeds input"))
}
/// Bound unordered extents, rejecting partial overlaps. Identical aliasing is
/// allowed only when the caller explicitly selects that preservation policy.
pub fn validate_extents(
    extents: &[Extent],
    scope: Range<u64>,
    alias: AliasPolicy,
    limits: &Limits,
    check: &dyn Fn() -> Result<()>,
) -> Result<()> {
    check()?;
    if scope.start > scope.end {
        return Err(malformed("Invalid ISO extent scope"));
    }
    if extents.len() > limits.extents
        || extents
            .len()
            .checked_mul(std::mem::size_of::<Range<u64>>())
            .is_none_or(|n| n > limits.metadata_bytes)
    {
        return Err(resource("ISO extent metadata exceeds the limit"));
    }
    let mut ranges = Vec::new();
    ranges
        .try_reserve_exact(extents.len())
        .map_err(|_| resource("ISO extent metadata allocation failed"))?;
    for extent in extents {
        check()?;
        let range = range(*extent, &scope)?;
        if !range.is_empty() {
            ranges.push(range);
        }
    }
    ranges.sort_unstable_by_key(|range| (range.start, range.end));
    for pair in ranges.windows(2) {
        check()?;
        if pair[1].start < pair[0].end && !(alias == AliasPolicy::AllowExact && pair[0] == pair[1])
        {
            return Err(malformed("ISO data extents overlap"));
        }
    }
    check()?;
    Ok(())
}
