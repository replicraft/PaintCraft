use super::*;
use ag_psd::psd::{BooleanOperation, FillRule};

pub(super) fn shape(
    source: &Shape,
    parsed: &ag_psd::csh::CshShape,
    check: &dyn Fn() -> Result<()>,
) -> Result<()> {
    let mut paths = parsed.mask.paths.iter();
    let mut knots = None;
    let mut clipboard = None;
    let mut initial = None;
    let fail = || invalid("CSH backend path geometry disagrees with preflight");
    for record in &source.records {
        check()?;
        match record {
            Record::Subpath {
                open,
                knots: count,
                operation,
                fill_rule,
            } => {
                let path = paths.next().ok_or_else(fail)?;
                let expected = match operation {
                    None => None,
                    Some(Operation::Exclude) => Some(BooleanOperation::Exclude),
                    Some(Operation::Combine) => Some(BooleanOperation::Combine),
                    Some(Operation::Subtract) => Some(BooleanOperation::Subtract),
                    Some(Operation::Intersect) => Some(BooleanOperation::Intersect),
                };
                if path.open != *open
                    || path.operation != expected
                    || path.knots.len() != usize::from(*count)
                    || path.fill_rule
                        != if *fill_rule == PathFillRule::NonZero {
                            FillRule::NonZero
                        } else {
                            FillRule::EvenOdd
                        }
                {
                    return Err(fail());
                }
                knots = Some(path.knots.iter());
            }
            Record::Knot { linked, points, .. } => {
                let knot = knots.as_mut().and_then(Iterator::next).ok_or_else(fail)?;
                let expected: Vec<_> = points
                    .iter()
                    .enumerate()
                    .map(|(i, value)| {
                        f64::from(*value) / 16777216.0
                            * f64::from(if i % 2 == 0 {
                                parsed.width
                            } else {
                                parsed.height
                            })
                    })
                    .collect();
                if knot.linked != *linked || knot.points != expected {
                    return Err(fail());
                }
            }
            Record::Clipboard(values) => {
                clipboard = Some(values.map(|v| f64::from(v) / 16777216.0))
            }
            Record::InitialFill(value) => initial = Some(*value),
            Record::FillRule(_) => {}
        }
    }
    let actual = parsed
        .mask
        .clipboard
        .as_ref()
        .map(|v| [v.top, v.left, v.bottom, v.right, v.resolution]);
    if actual != clipboard || parsed.mask.fill_starts_with_all_pixels != initial {
        return Err(fail());
    }
    Ok(())
}
