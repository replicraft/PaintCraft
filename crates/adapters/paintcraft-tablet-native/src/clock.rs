//! Checked native clock conversion; never saturate an invalid hardware time.
pub(crate) fn uptime_micros(seconds: f64) -> Option<u64> {
    let micros = seconds * 1_000_000.0;
    if !micros.is_finite() || micros < 0.0 || micros >= u64::MAX as f64 {
        None
    } else {
        Some(micros as u64)
    }
}
#[cfg(test)]
mod tests {
    #[test]
    fn uptime_conversion_refuses_nonfinite_negative_and_saturating_times() {
        assert_eq!(super::uptime_micros(0.0), Some(0));
        assert_eq!(super::uptime_micros(12.5), Some(12_500_000));
        for time in [f64::NAN, f64::INFINITY, -1.0, u64::MAX as f64 / 1_000_000.0] {
            assert_eq!(super::uptime_micros(time), None);
        }
    }
}
