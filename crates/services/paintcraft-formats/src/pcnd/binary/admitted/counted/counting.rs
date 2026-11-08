//! Private scalar sink candidate only. No source fact, broker or call-site activation.
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
    use paintcraft_core::types::{
        ErrorCode,
        resources::{AllocationClass, SharedBudget},
    };
    #[test]
    fn small_and_empty_writes_count_whole_input_without_payload_storage() {
        let budget = SharedBudget::new(4096);
        let scope = budget.scope(AllocationClass::PaintMetadata);
        let mut sink = CountingSink::new(&scope).unwrap();
        assert_eq!(sink.write(&[1, 2, 3]).unwrap(), 3);
        sink.write_all(&[4, 5]).unwrap();
        assert_eq!(sink.write(&[]).unwrap(), 0);
        sink.flush().unwrap();
        assert_eq!(sink.bytes(), 5);
        assert_eq!(sink.stop(), None);
        assert_eq!(scope.footprint().live, CountingSink::control_bytes());
        drop(sink);
        assert_eq!(scope.footprint().live, 0);
        assert_eq!(budget.usage().reserved, 0);
    }
    #[test]
    fn exact64mib_counts_and_next_byte_refuses_without_increment_or_reset() {
        let budget = SharedBudget::new(4096);
        let scope = budget.scope(AllocationClass::PaintMetadata);
        let mut sink = CountingSink::new(&scope).unwrap();
        for _ in 0..16384 {
            sink.write_all(&[0; 4096]).unwrap();
        }
        assert_eq!(sink.bytes(), 67108864);
        assert!(sink.write(&[1]).is_err());
        assert_eq!(sink.bytes(), 67108864);
        assert_eq!(sink.stop(), Some(CountStop::Limit));
        assert!(sink.write(&[]).is_err());
        assert!(sink.flush().is_err());
        assert_eq!(sink.bytes(), 67108864);
        drop(sink);
        assert_eq!(budget.usage().reserved, 0);
    }
    #[test]
    fn complete_write_refuses_before_count_crosses_limit() {
        let budget = SharedBudget::new(4096);
        let scope = budget.scope(AllocationClass::PaintMetadata);
        let mut sink = CountingSink::new(&scope).unwrap();
        // Child-module intervention avoids allocating a near64MiB input fixture.
        sink.bytes = SOURCE_MAX - 2;
        assert!(sink.write(&[0; 3]).is_err());
        assert_eq!(sink.bytes(), SOURCE_MAX - 2);
        assert_eq!(sink.stop(), Some(CountStop::Limit));
        assert!(sink.write(&[0]).is_err());
        assert_eq!(sink.bytes(), SOURCE_MAX - 2);
    }
    #[test]
    fn arithmetic_overflow_is_sticky_and_never_wraps_to_success() {
        let budget = SharedBudget::new(4096);
        let scope = budget.scope(AllocationClass::PaintMetadata);
        let mut sink = CountingSink::new(&scope).unwrap();
        // This state is unreachable through the production64MiB write API.
        sink.bytes = u64::MAX;
        assert!(sink.write(&[0]).is_err());
        assert_eq!(sink.bytes(), u64::MAX);
        assert_eq!(sink.stop(), Some(CountStop::Overflow));
        assert!(sink.write(&[]).is_err());
        assert_eq!(sink.stop(), Some(CountStop::Overflow));
    }
    #[test]
    fn control_exact_and_minus_one_keep_original_reservation() {
        let exact = CountingSink::control_bytes();
        for capacity in [exact - 1, exact] {
            let budget = SharedBudget::new(exact + 65536);
            let reservation = budget.reserve_capacity(capacity).unwrap();
            let scope = reservation.clone().scope(AllocationClass::PaintMetadata);
            let result = CountingSink::new(&scope);
            if capacity == exact {
                let sink = result.unwrap();
                assert_eq!(reservation.available(), 0);
                assert_eq!(scope.footprint().live, exact);
                drop(sink);
            } else {
                // No Debug bound or fabricated sink is needed for refusal.
                let error = match result {
                    Err(error) => error,
                    Ok(_) => panic!("underfunded"),
                };
                assert_eq!(error.code, ErrorCode::ResourceLimit);
            }
            assert_eq!(scope.footprint().live, 0);
            assert_eq!(reservation.available(), capacity);
            assert_eq!(budget.usage().reserved, capacity);
            drop(scope);
            drop(reservation);
            assert_eq!(budget.usage().reserved, 0);
        }
    }
}
