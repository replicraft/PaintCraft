//! Conservative same-domain source cutoffs. Equal clock values cannot prove
//! that a point followed a geometry/modifier change. This can reject valid
//! same-tick input; it is a refusal policy, not a responsive-runtime guarantee.
pub(super) struct Barrier {
    pub performance: u64,
    pub ticks: u64,
}
impl Barrier {
    pub fn admits_at(&self, performance: u64, ticks: u32, now_millis: u64) -> bool {
        if performance != 0 {
            performance > self.performance
        } else {
            super::clock::ticks(ticks, now_millis, 0).is_some_and(|t| t.micros / 1000 > self.ticks)
        }
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn equal_or_earlier_performance_count_cannot_prove_post_boundary_geometry() {
        let barrier = Barrier {
            performance: 100,
            ticks: 200,
        };
        assert!(!barrier.admits_at(99, 201, 201));
        assert!(!barrier.admits_at(100, 201, 201));
        assert!(barrier.admits_at(101, 0, 201));
    }
    #[test]
    fn equal_coarse_tick_is_ambiguous_even_when_receipt_time_is_later() {
        let barrier = Barrier {
            performance: 100,
            ticks: 200,
        };
        assert!(!barrier.admits_at(0, 199, 500));
        assert!(!barrier.admits_at(0, 200, 500));
        assert!(barrier.admits_at(0, 201, 500));
        assert!(!barrier.admits_at(0, 501, 500));
    }
    #[test]
    fn tick_wrap_keeps_the_source_cutoff_without_rebasing_or_sorting() {
        let barrier = Barrier {
            performance: 100,
            ticks: (1u64 << 32) + 1,
        };
        let now = (1u64 << 32) + 5;
        assert!(!barrier.admits_at(0, u32::MAX, now));
        assert!(!barrier.admits_at(0, 1, now));
        assert!(barrier.admits_at(0, 2, now));
    }
}
