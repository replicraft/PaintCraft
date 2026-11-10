//! Private Source8 complete-count sink; no payload or diagnostic heap storage.
use paintcraft_core::types::{
    Result,
    resources::{BudgetAccount, BudgetScope},
};
use std::io::{self, Write};
const SOURCE_MAX: u64 = 64 << 20;
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum CountStop {
    Limit,
    Overflow,
}
/// No payload/error String allocation. Existing admitted writer owns cancellation
/// and cumulative work before every sink write; this adds no callback or counter.
pub(super) struct CountingSink {
    bytes: u64,
    stopped: Option<CountStop>,
    _control: BudgetAccount,
}
impl CountingSink {
    pub(super) const fn control_bytes() -> u64 {
        std::mem::size_of::<Self>() as u64
    }
    pub(super) fn new(scope: &BudgetScope) -> Result<Self> {
        let mut control = scope.account();
        control.resize(Self::control_bytes())?;
        Ok(Self {
            bytes: 0,
            stopped: None,
            _control: control,
        })
    }
    pub(super) fn bytes(&self) -> u64 {
        self.bytes
    }
    pub(super) fn stop(&self) -> Option<CountStop> {
        self.stopped
    }
    fn refuse(&mut self, stop: CountStop) -> io::Error {
        if self.stopped.is_none() {
            self.stopped = Some(stop);
        }
        // ErrorKind conversion stores a simple kind, never a boxed custom error.
        io::ErrorKind::Other.into()
    }
}
impl Write for CountingSink {
    fn write(&mut self, input: &[u8]) -> io::Result<usize> {
        if let Some(stop) = self.stopped {
            return Err(self.refuse(stop));
        }
        let Some(next) = self.bytes.checked_add(input.len() as u64) else {
            return Err(self.refuse(CountStop::Overflow));
        };
        if next > SOURCE_MAX {
            return Err(self.refuse(CountStop::Limit));
        }
        self.bytes = next;
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

#[cfg(test)]
mod tests {
    use super::*;
    use paintcraft_core::types::resources::{AllocationClass, SharedBudget};
    #[test]
    fn source_limit_stop_is_sticky_without_an_output_allocation_or_count_wrap() {
        let budget = SharedBudget::new(4096);
        let scope = budget
            .reserve_capacity(4096)
            .unwrap()
            .scope(AllocationClass::PaintMetadata);
        let mut sink = CountingSink::new(&scope).unwrap();
        sink.bytes = SOURCE_MAX - 1;
        assert_eq!(sink.write(&[1]).unwrap(), 1);
        assert_eq!(sink.bytes(), SOURCE_MAX);
        assert!(sink.write(&[2]).is_err());
        assert_eq!(sink.bytes(), SOURCE_MAX);
        assert_eq!(sink.stop(), Some(CountStop::Limit));
        assert!(sink.write(&[]).is_err());
        assert!(sink.flush().is_err());
        drop(sink);
        assert_eq!(scope.footprint().live, 0);
    }
}
