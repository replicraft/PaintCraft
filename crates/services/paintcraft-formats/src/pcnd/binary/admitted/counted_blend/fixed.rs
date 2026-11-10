use super::*;
use std::io::{self, Write};
/// Retains storage before its unique original declaration. No detached Vec escape.
///
/// Storage cannot be detached from its assigned account:
/// ```compile_fail
/// use paintcraft_formats::pcnd::CountedBlendSnapshotBytes;
/// fn detach(bytes: CountedBlendSnapshotBytes) -> Vec<u8> {
///     bytes.into_vec()
/// }
/// ```
pub struct CountedBlendSnapshotBytes {
    bytes: Vec<u8>,
    _account: BudgetAccount,
    // Original factory outlives both data and its declaration, never a new role.
    _factory: BudgetScope,
}
impl std::fmt::Debug for CountedBlendSnapshotBytes {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("CountedBlendSnapshotBytes")
            .field("len", &self.len())
            .finish_non_exhaustive()
    }
}
impl CountedBlendSnapshotBytes {
    pub fn as_bytes(&self) -> &[u8] {
        &self.bytes
    }
    pub fn len(&self) -> usize {
        self.bytes.len()
    }
    pub fn is_empty(&self) -> bool {
        self.bytes.is_empty()
    }
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Stop {
    Overflow,
    Length,
}
struct FixedSink<'a> {
    owner: &'a mut CountedBlendSnapshotBytes,
    cursor: usize,
    stopped: Option<Stop>,
    _control: BudgetAccount,
}
pub(super) fn control_bytes() -> u64 {
    size_of::<FixedSink<'static>>() as u64
}
impl FixedSink<'_> {
    fn complete(&self) -> Result<()> {
        if self.stopped.is_some() || self.cursor != self.owner.bytes.len() {
            Err(mismatch())
        } else {
            Ok(())
        }
    }
    fn refuse(&mut self, stop: Stop) -> io::Error {
        if self.stopped.is_none() {
            self.stopped = Some(stop);
        }
        io::ErrorKind::Other.into()
    }
}
impl Write for FixedSink<'_> {
    fn write(&mut self, input: &[u8]) -> io::Result<usize> {
        if let Some(stop) = self.stopped {
            return Err(self.refuse(stop));
        }
        let Some(end) = self.cursor.checked_add(input.len()) else {
            return Err(self.refuse(Stop::Overflow));
        };
        if end > self.owner.bytes.len() {
            return Err(self.refuse(Stop::Length));
        }
        self.owner.bytes[self.cursor..end].copy_from_slice(input);
        self.cursor = end;
        Ok(input.len())
    }
    fn flush(&mut self) -> io::Result<()> {
        if let Some(stop) = self.stopped {
            Err(self.refuse(stop))
        } else {
            Ok(())
        }
    }
}
fn allocate(
    scope: &BudgetScope,
    bytes: u64,
    spend: &dyn Fn(u64) -> Result<()>,
) -> Result<(CountedBlendSnapshotBytes, BudgetAccount)> {
    if bytes > SOURCE_MAX {
        return Err(limit("counted snapshot exceeds source limit"));
    }
    let length = usize::try_from(bytes).map_err(|_| limit("snapshot length overflow"))?;
    let sizes = blend_counted_snapshot_control_bytes();
    let mut owner_account = scope.account();
    owner_account.resize(bytes.checked_add(sizes.bytes_owner).ok_or_else(mismatch)?)?;
    let mut sink_account = scope.account();
    sink_account.resize(sizes.fixed_sink)?;
    // Both declarations exist before the first allocation; deny work beforehand.
    spend(bytes)?;
    let mut owner = CountedBlendSnapshotBytes {
        bytes: Vec::new(),
        _account: owner_account,
        _factory: scope.clone(),
    };
    owner
        .bytes
        .try_reserve_exact(length)
        .map_err(|_| limit("counted snapshot allocation refused"))?;
    // Pinned default-Global Vec logical capacity proof, never an upward grant.
    if owner.bytes.capacity() != length {
        return Err(mismatch());
    }
    owner.bytes.resize(length, 0);
    Ok((owner, sink_account))
}
pub(super) fn encode(fact: &CountedBlendSnapshot<'_>) -> Result<CountedBlendSnapshotBytes> {
    (fact.spend)(64)?;
    let sizes = blend_counted_snapshot_control_bytes();
    (fact.spend)(2 * sizes.bytes_owner + 2 * sizes.fixed_sink + 160 + 5 * 128 + 64)?;
    validate_layouts()?;
    step(fact.check, fact.spend)?;
    let (mut owner, control) = allocate(fact.scope, fact.bytes, fact.spend)?;
    {
        let mut sink = FixedSink {
            owner: &mut owner,
            cursor: 0,
            stopped: None,
            _control: control,
        };
        let result = write_checked(fact.doc, &mut sink, fact.scope, fact.check, fact.spend);
        if sink.stopped.is_some() {
            return Err(mismatch());
        }
        result?;
        sink.complete()?;
    }
    Ok(owner)
}
pub(super) const fn return_layout_supported() -> bool {
    size_of::<Error>() <= 112
        && size_of::<Result<()>>() <= 128
        && size_of::<Result<(CountedBlendSnapshotBytes, BudgetAccount)>>() <= 160
}
#[cfg(test)]
mod tests;
