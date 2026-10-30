//! Allocation-free spatial-kernel foundation, not an editable document filter.
//! A caller admits all input/output/scratch storage before calling the kernel.
//! Cancellation may leave private output/scratch partial; discard it on failure.
use paintcraft_core::types::{Error, ErrorCode, Rect, Result, Rgba, filters::BoxBlurV1};

fn invalid() -> Error {
    Error::new(
        ErrorCode::InputSchema,
        "Invalid box blur geometry or storage",
    )
}

fn limited() -> Error {
    Error::new(ErrorCode::ResourceLimit, "Box blur storage quote overflows")
}

fn dimensions(rect: Rect) -> Result<(usize, usize)> {
    let width = rect.x1.checked_sub(rect.x0).filter(|v| *v > 0);
    let height = rect.y1.checked_sub(rect.y0).filter(|v| *v > 0);
    Ok((
        usize::try_from(width.ok_or_else(invalid)?).map_err(|_| limited())?,
        usize::try_from(height.ok_or_else(invalid)?).map_err(|_| limited())?,
    ))
}

fn footprint(source: Rect, area: Rect, policy: BoxBlurV1) -> Result<Rect> {
    dimensions(source)?;
    dimensions(area)?;
    if source.intersect(&area) != Some(area) {
        return Err(invalid());
    }
    // Clip before growing, using distances inside the validated source.
    // Never construct an overflowing virtual halo rectangle.
    let radius = i64::from(policy.radius());
    Ok(Rect {
        x0: area.x0 - radius.min(area.x0 - source.x0),
        y0: area.y0 - radius.min(area.y0 - source.y0),
        x1: area.x1 + radius.min(source.x1 - area.x1),
        y1: area.y1 + radius.min(source.y1 - area.y1),
    })
}

/// Conservative output footprint for a nonempty changed input rectangle wholly
/// inside the finite source. This is a distinct dirty-propagation operation;
/// callers must not substitute the unexpanded changed source rectangle.
pub fn box_affected_output(source: Rect, changed: Rect, policy: BoxBlurV1) -> Result<Rect> {
    footprint(source, changed, policy)
}

/// An exact checked quote, including the halo. The buffers remain caller-owned.
/// Scratch contains unrounded horizontal sums, not intermediate RGBA pixels.
#[derive(Clone, Copy, Debug)]
pub struct BoxBlurPlan {
    source: Rect,
    output: Rect,
    input: Rect,
    policy: BoxBlurV1,
    input_width: usize,
    output_width: usize,
    input_pixels: usize,
    output_pixels: usize,
    scratch_elements: usize,
    storage_bytes: usize,
    sample_visits: usize,
}

impl BoxBlurPlan {
    pub fn new(source: Rect, output: Rect, policy: BoxBlurV1) -> Result<Self> {
        let (output_width, output_height) = dimensions(output)?;
        let input = footprint(source, output, policy)?;
        let radius = i64::from(policy.radius());
        let (input_width, input_height) = dimensions(input)?;
        let input_pixels = input_width.checked_mul(input_height).ok_or_else(limited)?;
        let output_pixels = output_width
            .checked_mul(output_height)
            .ok_or_else(limited)?;
        let scratch_elements = if radius == 0 {
            0
        } else {
            output_width
                .checked_mul(input_height)
                .and_then(|v| v.checked_mul(4))
                .ok_or_else(limited)?
        };
        let storage_bytes = input_pixels
            .checked_add(output_pixels)
            .and_then(|v| v.checked_mul(4))
            .and_then(|v| v.checked_add(scratch_elements.checked_mul(8)?))
            .ok_or_else(limited)?;
        let sample_visits = if radius == 0 {
            output_pixels
        } else {
            let diameter = radius as usize * 2 + 1;
            let horizontal = (output_width - 1)
                .checked_mul(2)
                .and_then(|v| v.checked_add(diameter))
                .and_then(|v| v.checked_mul(input_height))
                .ok_or_else(limited)?;
            let vertical = (output_height - 1)
                .checked_mul(2)
                .and_then(|v| v.checked_add(diameter))
                .and_then(|v| v.checked_mul(output_width))
                .ok_or_else(limited)?;
            horizontal.checked_add(vertical).ok_or_else(limited)?
        };
        Ok(Self {
            source,
            output,
            input,
            policy,
            input_width,
            output_width,
            input_pixels,
            output_pixels,
            scratch_elements,
            storage_bytes,
            sample_visits,
        })
    }

    pub const fn required_input(self) -> Rect {
        self.input
    }
    pub const fn output(self) -> Rect {
        self.output
    }
    pub const fn input_pixels(self) -> usize {
        self.input_pixels
    }
    pub const fn output_pixels(self) -> usize {
        self.output_pixels
    }
    pub const fn scratch_elements(self) -> usize {
        self.scratch_elements
    }
    /// Logical simultaneous input + output + u64 scratch bytes. Allocation
    /// capacity, source storage, prefix/mask work and other jobs require separate
    /// caller-owned charges. This quote alone is not resource admission.
    pub const fn storage_bytes(self) -> usize {
        self.storage_bytes
    }

    /// Number of source-pixel and horizontal-aggregate visits by the two
    /// running-sum passes. A visit handles four channels. This is deterministic
    /// work accounting, not a CPU-time or cancellation-latency guarantee.
    pub const fn sample_visits(self) -> usize {
        self.sample_visits
    }

    fn input_index(self, x: i64, y: i64) -> usize {
        (y - self.input.y0) as usize * self.input_width + (x - self.input.x0) as usize
    }

    fn horizontal_index(self, x: usize, y: i64) -> usize {
        ((y - self.input.y0) as usize * self.output_width + x) * 4
    }

    fn tap_x(self, x: i64, offset: i64) -> i64 {
        // Derive source-local offsets first, so even extreme signed origins
        // cannot overflow when adding the radius to a document coordinate.
        let local = x - self.source.x0;
        let last = self.source.x1 - self.source.x0 - 1;
        self.source.x0
            + if offset >= 0 {
                local + offset.min(last - local)
            } else {
                local - (-offset).min(local)
            }
    }

    fn tap_y(self, y: i64, offset: i64) -> i64 {
        let local = y - self.source.y0;
        let last = self.source.y1 - self.source.y0 - 1;
        self.source.y0
            + if offset >= 0 {
                local + offset.min(last - local)
            } else {
                local - (-offset).min(local)
            }
    }
}

fn weighted(pixel: Rgba) -> [u64; 4] {
    let [r, g, b, a] = pixel.0.map(u64::from);
    [r * a, g * a, b * a, a]
}

fn round(numerator: u64, denominator: u64) -> u8 {
    ((numerator + denominator / 2) / denominator) as u8
}

/// Exact fixed-square CLAMP blur. Horizontal sliding sums are never rounded;
/// vertical sliding sums divide once. Alpha-zero source RGB contributes zero.
/// Radius zero copies hidden RGB exactly. No buffers are allocated internally.
pub fn box_blur(
    plan: BoxBlurPlan,
    input: &[Rgba],
    output: &mut [Rgba],
    scratch: &mut [u64],
    check: &dyn Fn() -> Result<()>,
) -> Result<()> {
    if input.len() != plan.input_pixels
        || output.len() != plan.output_pixels
        || scratch.len() < plan.scratch_elements
    {
        return Err(invalid());
    }
    check()?;
    let radius = i64::from(plan.policy.radius());
    if radius == 0 {
        for (source, target) in input.chunks(64).zip(output.chunks_mut(64)) {
            check()?;
            target.copy_from_slice(source);
        }
        return check();
    }
    let horizontal = &mut scratch[..plan.scratch_elements];
    for y in plan.input.y0..plan.input.y1 {
        check()?;
        let mut sum = [0; 4];
        for dx in -radius..=radius {
            let pixel = weighted(input[plan.input_index(plan.tap_x(plan.output.x0, dx), y)]);
            for c in 0..4 {
                sum[c] += pixel[c];
            }
        }
        for (x, global_x) in (plan.output.x0..plan.output.x1).enumerate() {
            if x.is_multiple_of(64) {
                check()?;
            }
            let index = plan.horizontal_index(x, y);
            horizontal[index..index + 4].copy_from_slice(&sum);
            if global_x + 1 < plan.output.x1 {
                let removed = weighted(input[plan.input_index(plan.tap_x(global_x, -radius), y)]);
                let added = weighted(input[plan.input_index(plan.tap_x(global_x, radius + 1), y)]);
                for c in 0..4 {
                    sum[c] = sum[c] - removed[c] + added[c];
                }
            }
        }
    }
    check()?;
    let taps = (radius as u64 * 2 + 1).pow(2);
    for x in 0..plan.output_width {
        check()?;
        let mut sum = [0; 4];
        for dy in -radius..=radius {
            let index = plan.horizontal_index(x, plan.tap_y(plan.output.y0, dy));
            for c in 0..4 {
                sum[c] += horizontal[index + c];
            }
        }
        for (y, global_y) in (plan.output.y0..plan.output.y1).enumerate() {
            if y.is_multiple_of(64) {
                check()?;
            }
            output[y * plan.output_width + x] = if sum[3] == 0 {
                Rgba([0; 4])
            } else {
                Rgba([
                    round(sum[0], sum[3]),
                    round(sum[1], sum[3]),
                    round(sum[2], sum[3]),
                    round(sum[3], taps),
                ])
            };
            if global_y + 1 < plan.output.y1 {
                let removed = plan.horizontal_index(x, plan.tap_y(global_y, -radius));
                let added = plan.horizontal_index(x, plan.tap_y(global_y, radius + 1));
                for c in 0..4 {
                    sum[c] = sum[c] - horizontal[removed + c] + horizontal[added + c];
                }
            }
        }
    }
    check()
}

/// Apply spatial coverage once in premultiplied encoded-RGB space. Both exact
/// endpoints retain all straight RGBA bytes. If both alphas are zero, interpolate
/// hidden RGB directly; otherwise alpha-weight the two RGB tuples.
pub fn mix_spatial_coverage(incoming: Rgba, candidate: Rgba, coverage: u8) -> Rgba {
    if coverage == 0 {
        return incoming;
    }
    if coverage == 255 {
        return candidate;
    }
    let m = u64::from(coverage);
    let a = u64::from(incoming.0[3]);
    let b = u64::from(candidate.0[3]);
    let weight = (255 - m) * a + m * b;
    let mut output = [0; 4];
    for (c, channel) in output[..3].iter_mut().enumerate() {
        let p = u64::from(incoming.0[c]);
        let q = u64::from(candidate.0[c]);
        *channel = if weight == 0 {
            round((255 - m) * p + m * q, 255)
        } else {
            round((255 - m) * p * a + m * q * b, weight)
        };
    }
    output[3] = round(weight, 255);
    Rgba(output)
}

#[cfg(test)]
mod tests;

#[cfg(test)]
mod oracles;
