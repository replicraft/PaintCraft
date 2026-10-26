//! Validate encoded consumption and work bounds without reconstructing pixels.
use super::*;
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Summary {
    pub new_rle_rows: u32,
    pub legacy_rows: u32,
    pub packets: u64,
}
struct Bytes<'a> {
    bytes: &'a [u8],
    at: usize,
    summary: Summary,
    limit: u64,
}
impl<'a> Bytes<'a> {
    fn take(&mut self, count: usize) -> Result<&'a [u8]> {
        let end = self
            .at
            .checked_add(count)
            .ok_or_else(|| invalid("Radiance packet extent overflow"))?;
        let bytes = self
            .bytes
            .get(self.at..end)
            .ok_or_else(|| invalid("Truncated Radiance pixel packet"))?;
        self.at = end;
        Ok(bytes)
    }
    fn packet(&mut self, check: &dyn Fn() -> Result<()>) -> Result<()> {
        check()?;
        self.summary.packets += 1;
        if self.summary.packets > self.limit {
            return Err(resource(
                "Radiance pixel packet count exceeds work allowance",
            ));
        }
        Ok(())
    }
}
pub(super) fn validate(
    bytes: &[u8],
    width: u32,
    height: u32,
    check: &dyn Fn() -> Result<()>,
) -> Result<Summary> {
    let mut input = Bytes {
        bytes,
        at: 0,
        summary: Summary::default(),
        limit: 5 * u64::from(width) * u64::from(height),
    };
    for _ in 0..height {
        input.packet(check)?;
        let first: [u8; 4] = input.take(4)?.try_into().unwrap();
        let marker = first[0] == 2 && first[1] == 2 && first[2] < 128;
        if marker && (8..=32767).contains(&width) {
            let encoded = u32::from(u16::from_be_bytes([first[2], first[3]]));
            if encoded != width {
                return Err(invalid("Radiance RLE row width disagrees with resolution"));
            }
            for _ in 0..4 {
                let mut position = 0u32;
                while position < width {
                    input.packet(check)?;
                    let count = input.take(1)?[0];
                    if count == 0 {
                        return Err(invalid("Radiance component RLE has a zero-length packet"));
                    }
                    let produced = if count > 128 {
                        u32::from(count - 128)
                    } else {
                        u32::from(count)
                    };
                    if produced > width - position {
                        return Err(invalid("Radiance component packet exceeds row width"));
                    }
                    input.take(if count > 128 { 1 } else { usize::from(count) })?;
                    position += produced;
                }
            }
            input.summary.new_rle_rows += 1;
        } else {
            if marker {
                return Err(unsupported(
                    "Legacy Radiance row prefix is unqualified for the selected decoder",
                ));
            }
            if first[..3] == [1, 1, 1] {
                return Err(invalid("Radiance old RLE cannot begin with a repeat"));
            }
            let mut position = 1u32;
            let mut shift = 0;
            while position < width {
                input.packet(check)?;
                let pixel = input.take(4)?;
                if pixel[..3] == [1, 1, 1] {
                    // Canonical old runs need at most two base-256 digits under
                    // the admitted 65536-side bound. Reject useless zero chains
                    // before the selected decoder's multiplier can overflow.
                    if shift > 8 {
                        return Err(unsupported(
                            "Radiance legacy run digit chain exceeds qualified profile",
                        ));
                    }
                    let count = u32::from(pixel[3]) << shift;
                    if count > width - position {
                        return Err(invalid("Radiance old RLE exceeds row width"));
                    }
                    position += count;
                    shift += 8;
                } else {
                    position += 1;
                    shift = 0;
                }
            }
            input.summary.legacy_rows += 1;
        }
    }
    if input.at != bytes.len() {
        return Err(unsupported(
            "Radiance trailing data requires a separate interpretation",
        ));
    }
    check()?;
    Ok(input.summary)
}
