//! Portable snapshot compression: unchanged RLE/LZ4 choice, caller-owned storage.
use super::*;
use paintcraft_core::types::resources::{BudgetAccount, BudgetScope};

pub(in crate::pcnd) struct Buffer {
    bytes: Vec<u8>,
    _account: BudgetAccount,
}
impl Buffer {
    pub fn new(length: usize, scope: &BudgetScope) -> Result<Self> {
        let mut account = scope.account();
        account.resize(length as u64)?;
        let mut bytes = Vec::new();
        bytes.try_reserve_exact(length).map_err(|_| exhausted())?;
        if let Err(error) = account.resize(bytes.capacity() as u64) {
            drop(bytes);
            return Err(error);
        }
        bytes.resize(length, 0);
        Ok(Self {
            bytes,
            _account: account,
        })
    }
    pub fn bytes(&self) -> &[u8] {
        &self.bytes
    }
}
pub(in crate::pcnd) enum Encoded<'a> {
    Raw(&'a [u8]),
    Owned(&'static str, Buffer),
}
impl Encoded<'_> {
    pub fn encoding(&self) -> &'static str {
        match self {
            Self::Raw(_) => "raw",
            Self::Owned(kind, _) => kind,
        }
    }
    pub fn bytes(&self) -> &[u8] {
        match self {
            Self::Raw(bytes) => bytes,
            Self::Owned(_, b) => b.bytes(),
        }
    }
}
/// Fixed table payload upper for pinned lz4_flex0.14, including an alloc-feature
/// union's large4096*u32 table. This is a declaration, not allocator/RSS evidence.
const LZ4_TABLE_BYTES: u64 = 4096 * 4;

fn pack(
    data: &[u8],
    prefix: bool,
    scope: &BudgetScope,
    check: &dyn Fn() -> Result<()>,
    spend: &dyn Fn(u64) -> Result<()>,
) -> Result<Buffer> {
    check()?;
    let maximum = lz4_flex::block::get_maximum_output_size(data.len());
    let extra = usize::from(prefix) * 4;
    let length = maximum.checked_add(extra).ok_or_else(exhausted)?;
    // Named opaque-codec model: input bytes + initialized output upper + fixed
    // table slots. No instruction/time or global real-cost equivalence claim.
    spend(
        (data.len() as u64)
            .checked_add(length as u64)
            .and_then(|n| n.checked_add(4096))
            .ok_or_else(exhausted)?,
    )?;
    let mut table = scope.account();
    table.resize(LZ4_TABLE_BYTES)?;
    let mut output = Buffer::new(length, scope)?;
    if prefix {
        output.bytes[..4].copy_from_slice(&(data.len() as u32).to_le_bytes());
    }
    let used = lz4_flex::block::compress_into(data, &mut output.bytes[extra..])
        .map_err(|_| codec("LZ4 encoding failed"))?;
    output.bytes.truncate(extra + used);
    check()?;
    Ok(output)
}
/// Both callers and returned owned buffers retain their original scope accounts.
/// No native Zstandard path: exact portable (Wasm) encoder semantics on all targets.
pub(in crate::pcnd) fn block<'a>(
    data: &'a [u8],
    scope: &BudgetScope,
    check: &dyn Fn() -> Result<()>,
    spend: &dyn Fn(u64) -> Result<()>,
) -> Result<Encoded<'a>> {
    check()?;
    if data.is_empty() || data.len() > BLOCK_BYTES_MAX as usize || !data.len().is_multiple_of(4) {
        return Err(codec("invalid admitted snapshot block"));
    }
    let pixels = data.as_chunks::<4>().0;
    // First pass counts exact runs without allocating. One comparison per pixel.
    spend(pixels.len() as u64)?;
    let mut runs_count = 0usize;
    let mut at = 0;
    while at < pixels.len() {
        check()?;
        let mut count = 1usize;
        while count < 65_536 && at + count < pixels.len() && pixels[at + count] == pixels[at] {
            count += 1;
        }
        runs_count += 1;
        at += count;
    }
    let length = runs_count.checked_mul(6).ok_or_else(exhausted)?;
    // Second pass repeats comparisons and writes exactly six bytes per run.
    spend(
        (pixels.len() as u64)
            .checked_add(length as u64)
            .ok_or_else(exhausted)?,
    )?;
    let mut runs = Buffer::new(length, scope)?;
    at = 0;
    let mut out = 0;
    while at < pixels.len() {
        check()?;
        let mut count = 1usize;
        while count < 65_536 && at + count < pixels.len() && pixels[at + count] == pixels[at] {
            count += 1;
        }
        runs.bytes[out..out + 2].copy_from_slice(&((count - 1) as u16).to_le_bytes());
        runs.bytes[out + 2..out + 6].copy_from_slice(&pixels[at]);
        out += 6;
        at += count;
    }
    let packed_runs = pack(runs.bytes(), true, scope, check, spend)?;
    let mut best = if runs.bytes.len() < data.len() {
        Encoded::Owned("rgba-runs", runs)
    } else {
        Encoded::Raw(data)
    };
    if packed_runs.bytes.len() < best.bytes().len() {
        best = Encoded::Owned("runs-lz4", packed_runs);
    }
    let packed = pack(data, false, scope, check, spend)?;
    if packed.bytes.len() < best.bytes().len() {
        best = Encoded::Owned("lz4", packed);
    }
    check()?;
    Ok(best)
}
fn exhausted() -> Error {
    Error::new(
        ErrorCode::ResourceLimit,
        "admitted snapshot storage exhausted",
    )
}
