//! Native clock values stay in their declared domain; no cross-device sorting.
use paintcraft_input::tablet::{Clock, Time};

pub(super) fn performance(count: u64, frequency: u64, epoch: u64) -> Option<Time> {
    if count == 0 || frequency == 0 {
        return None;
    }
    let micros = u128::from(count).checked_mul(1_000_000)? / u128::from(frequency);
    Some(Time {
        clock: Clock::WindowsPerformance,
        epoch,
        micros: micros.try_into().ok()?,
    })
}

/// Interpret the OS's wrapping 32-bit tick value relative to an actual native
/// GetTickCount64 observation. More than half a wrap is ambiguous, not a rebase.
pub(super) fn ticks(value: u32, now_millis: u64, epoch: u64) -> Option<Time> {
    let age = (now_millis as u32).wrapping_sub(value);
    if age > i32::MAX as u32 {
        return None;
    }
    Some(Time {
        clock: Clock::WindowsTicks,
        epoch,
        micros: now_millis.checked_sub(u64::from(age))?.checked_mul(1000)?,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn clocks_are_checked_and_ticks_keep_the_actual_wrap() {
        assert_eq!(performance(12_345, 1000, 7).unwrap().micros, 12_345_000);
        assert!(performance(1, 0, 7).is_none());
        assert!(performance(0, 1000, 7).is_none());
        assert!(performance(u64::MAX, 1, 7).is_none());
        let now = (1u64 << 32) + 20;
        assert_eq!(
            ticks(u32::MAX - 4, now, 7).unwrap().micros,
            ((1u64 << 32) - 5) * 1000
        );
        assert!(ticks(21, now, 7).is_none());
        assert!(ticks(0, u64::MAX, 7).is_none());
    }
}
