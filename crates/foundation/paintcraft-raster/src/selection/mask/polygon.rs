//! Exact rational intersections and fallible heap sorting at document pixel centers.
use super::*;
use std::cmp::Ordering;
const VERTICES_MAX: usize = 65_536;
#[derive(Clone, Copy)]
struct Crossing {
    numerator: i128,
    denominator: i128,
}
impl Crossing {
    fn cmp(self, other: Self) -> Ordering {
        let (a, b) = (
            self.numerator.div_euclid(self.denominator),
            other.numerator.div_euclid(other.denominator),
        );
        a.cmp(&b).then_with(|| {
            // Remainders and denominators are bounded by 4*coordinate_abs_max;
            // their product fits i128 even when full rational cross-products do not.
            (self.numerator.rem_euclid(self.denominator) * other.denominator)
                .cmp(&(other.numerator.rem_euclid(other.denominator) * self.denominator))
        })
    }
    fn pixel_start(self) -> i64 {
        let n = self.numerator * 2 - self.denominator;
        let d = self.denominator * 2;
        // ceil(intersection - 1/2), keeping centers exactly on a left edge.
        (n.div_euclid(d) + i128::from(n.rem_euclid(d) != 0)) as i64
    }
}
pub(super) fn rasterize(
    domain: Rect,
    vertices: &[(i64, i64)],
    budget: &SharedBudget,
    limits: MaskLimits,
    check: &dyn Fn() -> Result<()>,
) -> Result<BinaryMask> {
    check()?;
    valid_domain(domain)?;
    if vertices.len() > VERTICES_MAX {
        return Err(limit());
    }
    let mut build = builder::Builder::new(domain, false, budget, limits)?;
    build.spend(vertices.len() as u64)?;
    for (i, &(x, y)) in vertices.iter().enumerate() {
        if i.is_multiple_of(256) {
            check()?;
        }
        if [x, y]
            .iter()
            .any(|n| n.unsigned_abs() > COORD_ABS_MAX as u64)
        {
            return Err(schema("polygon vertex exceeds selection coordinate bounds"));
        }
    }
    if vertices.len() < 3 {
        return build.finish(check);
    }
    let y0 = vertices.iter().map(|p| p.1).min().unwrap().max(domain.y0);
    let y1 = vertices.iter().map(|p| p.1).max().unwrap().min(domain.y1);
    let scan = (y1 - y0).max(0) as u64;
    if scan == 0
        || vertices.iter().all(|p| p.0 <= domain.x0)
        || vertices.iter().all(|p| p.0 >= domain.x1)
    {
        return build.finish(check);
    }
    build.spend(scan.checked_mul(vertices.len() as u64).ok_or_else(limit)?)?;
    let mut charge = build.account();
    charge.resize((vertices.len() * std::mem::size_of::<Crossing>()) as u64)?;
    let mut intersections = Vec::new();
    intersections
        .try_reserve_exact(vertices.len())
        .map_err(|_| limit())?;
    if let Err(error) =
        charge.resize((intersections.capacity() * std::mem::size_of::<Crossing>()) as u64)
    {
        drop(intersections);
        return Err(error);
    }
    for y in y0..y1 {
        check()?;
        intersections.clear();
        for i in 0..vertices.len() {
            if i.is_multiple_of(256) {
                check()?;
            }
            let (mut a, mut b) = (vertices[i], vertices[(i + 1) % vertices.len()]);
            if a.1 > b.1 {
                std::mem::swap(&mut a, &mut b);
            }
            let center = 2 * i128::from(y) + 1;
            if center < 2 * i128::from(a.1) || center >= 2 * i128::from(b.1) {
                continue;
            }
            let dy = i128::from(b.1) - i128::from(a.1);
            let dx = i128::from(b.0) - i128::from(a.0);
            intersections.push(Crossing {
                numerator: 2 * i128::from(a.0) * dy + (center - 2 * i128::from(a.1)) * dx,
                denominator: 2 * dy,
            });
        }
        sort(&mut intersections, &mut build, check)?;
        if !intersections.len().is_multiple_of(2) {
            return Err(Error::new(
                ErrorCode::InvariantViolation,
                "odd polygon intersection count",
            ));
        }
        for pair in intersections.as_chunks::<2>().0 {
            let left = pair[0].pixel_start().max(domain.x0);
            let right = pair[1].pixel_start().min(domain.x1);
            if left < right {
                build.span(
                    MaskSpan {
                        y,
                        x0: left,
                        x1: right - 1,
                        coverage: 255,
                    },
                    check,
                )?;
            }
        }
    }
    drop(intersections);
    drop(charge);
    build.finish(check)
}
fn sort(
    values: &mut [Crossing],
    build: &mut builder::Builder,
    check: &dyn Fn() -> Result<()>,
) -> Result<()> {
    let len = values.len();
    for root in (0..len / 2).rev() {
        sift(values, root, len, build, check)?;
    }
    for end in (1..len).rev() {
        check()?;
        values.swap(0, end);
        sift(values, 0, end, build, check)?;
    }
    Ok(())
}
fn sift(
    values: &mut [Crossing],
    mut root: usize,
    end: usize,
    build: &mut builder::Builder,
    check: &dyn Fn() -> Result<()>,
) -> Result<()> {
    while root * 2 + 1 < end {
        check()?;
        build.spend(2)?;
        let mut child = root * 2 + 1;
        if child + 1 < end && values[child].cmp(values[child + 1]) == Ordering::Less {
            child += 1;
        }
        if values[root].cmp(values[child]) != Ordering::Less {
            break;
        }
        values.swap(root, child);
        root = child;
    }
    Ok(())
}
