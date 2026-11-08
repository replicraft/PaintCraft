//! Named bounded metadata policy slots, never timing or instruction bounds.
use super::*;
pub(super) const ENTRY: u64 = 64;
pub(super) const ROOT: u64 = 2048;
pub(super) const FILTER: u64 = 16;
pub(super) fn add(a: u64, b: u64) -> Result<u64> {
    a.checked_add(b).ok_or_else(overflow)
}
pub(super) fn mul(a: u64, b: u64) -> Result<u64> {
    a.checked_mul(b).ok_or_else(overflow)
}
pub(super) fn eq(n: usize, width: u64) -> Result<u64> {
    mul(n as u64, width + 2)
}
pub(super) fn duplicates(n: usize, width: u64) -> Result<u64> {
    eq(
        n.checked_mul(n.saturating_sub(1)).ok_or_else(overflow)? / 2,
        width,
    )
}
pub(super) fn entry(roots: usize) -> Result<u64> {
    add(ENTRY, mul(ROOT, roots as u64)?)
}
/// Existing bound payloads, deduplicated ONLY by actual owner identity.
pub(super) fn owners<'a>(
    explicit: &'a WorkspaceAdmission,
    ambient: Option<&'a WorkspaceAdmission>,
) -> [Option<&'a WorkspaceAdmission>; 2] {
    let first = explicit.has_prepared_programs().then_some(explicit);
    let second = ambient.filter(|a| a.has_prepared_programs());
    [
        first,
        second.filter(|a| !first.is_some_and(|b| same_owner(a, b))),
    ]
}
pub(super) fn same_owner(a: &WorkspaceAdmission, b: &WorkspaceAdmission) -> bool {
    if let (Some(a), Some(b)) = (a.prepared_curves(), b.prepared_curves()) {
        return std::ptr::eq(a, b);
    }
    if a.prepared_curves().is_some() || b.prepared_curves().is_some() {
        return false;
    }
    if let (Some(a), Some(b)) = (a.prepared_programs(), b.prepared_programs()) {
        return std::ptr::eq(a, b);
    }
    if a.prepared_programs().is_some() || b.prepared_programs().is_some() {
        return false;
    }
    match (a.prepared_levels(), b.prepared_levels()) {
        (Some(a), Some(b)) => std::ptr::eq(a, b),
        _ => false,
    }
}
pub(super) fn lengths(a: &WorkspaceAdmission) -> [usize; 3] {
    let l = a.prepared_levels().map_or(0, PreparedLevelsJob::len);
    let p = a.prepared_programs().map_or(0, PreparedPointJob::len);
    let c = a.prepared_curves().map_or(0, PreparedCurvesJob::len);
    [l, p.saturating_sub(l), c.saturating_sub(p)]
}
pub(super) fn validation(filters: u64, occurrences: [u64; 3], keys: [usize; 3]) -> Result<u64> {
    let mut n = mul(3 * FILTER, filters)?;
    for ((count, keys), width) in occurrences.into_iter().zip(keys).zip([6, 2, 5]) {
        n = add(n, mul(count, eq(keys, width)?)?)?;
    }
    Ok(n)
}
pub(super) fn family_quote(n: usize, width: u64, scalar: u64) -> Result<u64> {
    add(add(duplicates(n, width)?, mul(n as u64, scalar + 64)?)?, 64)
}
/// Sum of nested quote helpers actually called by the selected branch.
pub(super) fn quote_metadata(n: [usize; 3]) -> Result<u64> {
    let [l, e, c] = n;
    if l + e + c == 0 {
        return Ok(0);
    }
    let mut total = family_quote(l, 6, 1088)?;
    if e != 0 || c != 0 {
        total = add(total, family_quote(e, 2, 576)?)?;
    }
    if c != 0 {
        total = add(total, family_quote(c, 5, 64)?)?;
    }
    Ok(total)
}
/// Constructor metadata AFTER the leaf's initial quote was already paid.
pub(super) fn constructor_metadata(n: [usize; 3]) -> Result<u64> {
    let [l, e, c] = n;
    if l + e + c == 0 {
        return Ok(0);
    }
    let ql = family_quote(l, 6, 1088)?;
    let qe = family_quote(e, 2, 576)?;
    let qc = family_quote(c, 5, 64)?;
    let mut total = if c != 0 {
        // Curves quote, optional Point quote, optional Levels quote.
        add(add(ql, qe)?, qc)?
    } else if e != 0 {
        add(ql, qe)?
    } else {
        ql
    };
    if c != 0 && l + e != 0 {
        total = add(total, add(ql, qe)?)?;
    }
    if (c != 0 || e != 0) && l != 0 {
        total = add(total, ql)?;
    }
    for ((count, width), scalar) in n.into_iter().zip([6, 2, 5]).zip([2176, 1664, 192]) {
        total = add(
            total,
            add(duplicates(count, width)?, mul(count as u64, scalar)?)?,
        )?;
    }
    Ok(total)
}
