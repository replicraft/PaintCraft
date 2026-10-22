//! Resolved, ordered and disjoint grayscale coverage. Each row is painted once.
use super::*;
use paintcraft_core::types::text::MaskSpan;

pub fn validate_mask_spans(spans: &[MaskSpan]) -> Result<()> {
    validate_mask_spans_checked(spans, &|| Ok(()))
}

/// Validate resolved coverage with cooperative checks, including offscreen rows.
/// The parser uses the same policy without a cancellation token.
pub fn validate_mask_spans_checked(
    spans: &[MaskSpan],
    check: &dyn Fn() -> Result<()>,
) -> Result<()> {
    check()?;
    if spans.is_empty() || spans.len() > 262_144 {
        return Err(bad("mask needs 1..=262144 spans"));
    }
    let mut previous: Option<MaskSpan> = None;
    let mut pixels = 0i128;
    for (index, &span) in spans.iter().enumerate() {
        if index.is_multiple_of(1024) {
            check()?;
        }
        if span.x0.unsigned_abs() > COORD_ABS_MAX as u64
            || span.x1.unsigned_abs() > COORD_ABS_MAX as u64
            || span.y.unsigned_abs() > COORD_ABS_MAX as u64
            || span.x0 > span.x1
            || span.coverage == 0
        {
            return Err(bad("invalid mask span"));
        }
        if previous.is_some_and(|p| span.y < p.y || (span.y == p.y && span.x0 <= p.x1)) {
            return Err(bad("mask spans must be sorted and disjoint"));
        }
        pixels += i128::from(span.x1) - i128::from(span.x0) + 1;
        if pixels > 16_777_216 {
            return Err(bad("mask pixel budget exceeded"));
        }
        previous = Some(span);
    }
    check()
}

pub(super) fn parse(v: &Value) -> Result<Op> {
    let o = obj(v, &["op", "layer_ref", "color", "spans"], &[])?;
    let rows = o["spans"]
        .as_array()
        .ok_or_else(|| bad("spans must be an array"))?;
    if rows.is_empty() || rows.len() > 262_144 {
        return Err(bad("mask needs 1..=262144 spans"));
    }
    let spans = rows
        .iter()
        .map(|row| {
            let row = row
                .as_array()
                .filter(|r| r.len() == 4)
                .ok_or_else(|| bad("mask span must be [x0,y,x1,coverage]"))?;
            let x0 = int_in(&row[0], -COORD_ABS_MAX, COORD_ABS_MAX, "mask x0")?;
            Ok(MaskSpan {
                x0,
                y: int_in(&row[1], -COORD_ABS_MAX, COORD_ABS_MAX, "mask y")?,
                x1: int_in(&row[2], x0, COORD_ABS_MAX, "mask x1")?,
                coverage: int_in(&row[3], 1, 255, "mask coverage")? as u8,
            })
        })
        .collect::<Result<Vec<_>>>()?;
    validate_mask_spans(&spans)?;
    Ok(Op::MaskSpans {
        layer: layer_ref(&o["layer_ref"])?,
        color: color(&o["color"])?,
        spans,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn resolved_mask_validation_cooperates_at_every_observed_checkpoint() {
        use std::cell::Cell;
        let spans: Vec<_> = (0..4096)
            .map(|y| MaskSpan {
                y,
                x0: 0,
                x1: 0,
                coverage: 255,
            })
            .collect();
        let total = Cell::new(0);
        validate_mask_spans_checked(&spans, &|| {
            total.set(total.get() + 1);
            Ok(())
        })
        .unwrap();
        for cancel in 1..=total.get() {
            let calls = Cell::new(0);
            let error = validate_mask_spans_checked(&spans, &|| {
                calls.set(calls.get() + 1);
                if calls.get() == cancel {
                    Err(Error::new(ErrorCode::OperationCancelled, "cancel mask"))
                } else {
                    Ok(())
                }
            })
            .unwrap_err();
            assert_eq!(error.code, ErrorCode::OperationCancelled);
            assert_eq!(calls.get(), cancel);
        }
    }
    #[test]
    fn masks_reject_overlap_unsorted_rows_overflow_and_unbounded_work() {
        let span = MaskSpan {
            y: 0,
            x0: 0,
            x1: 4,
            coverage: 128,
        };
        for invalid in [
            vec![],
            vec![span, span],
            vec![span, MaskSpan { y: -1, ..span }],
            vec![MaskSpan {
                x1: 16_777_216,
                ..span
            }],
            vec![MaskSpan {
                x0: i64::MIN,
                ..span
            }],
            vec![MaskSpan { x1: -1, ..span }],
            vec![MaskSpan {
                coverage: 0,
                ..span
            }],
        ] {
            assert!(validate_mask_spans(&invalid).is_err());
        }
        assert!(
            validate_mask_spans(&[
                span,
                MaskSpan {
                    x0: 5,
                    x1: 9,
                    ..span
                }
            ])
            .is_ok()
        );
    }
}
