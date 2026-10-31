use super::*;

/// A measured heuristic, not a universal performance guarantee. The u32 builder
/// remains an explicit reference API; this choice changes no numerical profile.
const WIDE_SELECTION_WORK: u64 = 1_000_000;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum LevelsBackend {
    U32,
    U64,
}

/// Raw counters have different word widths. They are neither interchangeable
/// quota units nor elapsed-time measurements.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CompileWork {
    U32 { units: u64 },
    U64 { units: u64 },
}

pub(super) fn quote(policy: LevelsRgbV1) -> LevelsQuote {
    let narrow = levels::quote(policy);
    let wide = levels::u64::quote(policy);
    let backend = if narrow.work >= WIDE_SELECTION_WORK {
        LevelsBackend::U64
    } else {
        LevelsBackend::U32
    };
    let scratch_bytes = match backend {
        LevelsBackend::U32 => narrow.scratch_bytes,
        LevelsBackend::U64 => wide.scratch_bytes,
    };
    LevelsQuote {
        backend,
        peak_bytes: scratch_bytes + LEVELS_TABLE_BYTES,
        retained_bytes: LEVELS_TABLE_BYTES,
        scratch_bytes,
        // A conservative abstract preparation allowance bounds either raw
        // counter. This does not claim that u32 and u64 operations cost equally.
        compile_work: narrow.work.max(wide.work),
    }
}

pub(super) fn build(
    backend: LevelsBackend,
    policy: LevelsRgbV1,
    table: &mut [u8; 256],
    check: &dyn Fn() -> Result<()>,
) -> Result<CompileWork> {
    match backend {
        LevelsBackend::U32 => narrow(policy, table, check),
        LevelsBackend::U64 => wide(policy, table, check),
    }
}

// Separate frames construct exactly the selected caller-owned scratch. No
// scratch enum is switched/copied and no second full-width workspace is live.
#[inline(never)]
fn narrow(
    policy: LevelsRgbV1,
    table: &mut [u8; 256],
    check: &dyn Fn() -> Result<()>,
) -> Result<CompileWork> {
    let mut scratch = levels::Scratch::default();
    let used = levels::build_lut(
        policy,
        &mut scratch,
        table,
        levels::Limits::from(levels::quote(policy)),
        check,
    )?;
    Ok(CompileWork::U32 { units: used.units })
}

#[inline(never)]
fn wide(
    policy: LevelsRgbV1,
    table: &mut [u8; 256],
    check: &dyn Fn() -> Result<()>,
) -> Result<CompileWork> {
    let mut scratch = levels::u64::Scratch::default();
    let used = levels::u64::build_lut(
        policy,
        &mut scratch,
        table,
        levels::u64::Limits::from(levels::u64::quote(policy)),
        check,
    )?;
    Ok(CompileWork::U64 { units: used.units })
}
