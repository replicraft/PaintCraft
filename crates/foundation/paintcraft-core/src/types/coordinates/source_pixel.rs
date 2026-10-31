//! Integer SOURCE-local addressing, deliberately unrelated to document points.
use crate::types::{Error, ErrorCode, Result};
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct SourcePixel {
    x: u32,
    y: u32,
}
impl SourcePixel {
    /// The caller separately proves membership in its captured layer bounds.
    pub const fn new(x: u32, y: u32) -> Self {
        Self { x, y }
    }
    pub const fn x(self) -> u32 {
        self.x
    }
    pub const fn y(self) -> u32 {
        self.y
    }
    pub fn try_from_signed(x: i64, y: i64) -> Result<Self> {
        let convert = |n| {
            u32::try_from(n).map_err(|_| {
                Error::new(
                    ErrorCode::InputSchema,
                    "Source pixel requires unsigned32-bit local coordinates",
                )
            })
        };
        Ok(Self::new(convert(x)?, convert(y)?))
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn signed_conversion_never_wraps_or_implies_document_rebasing() {
        let p = SourcePixel::try_from_signed(0, i64::from(u32::MAX)).unwrap();
        assert_eq!((p.x(), p.y()), (0, u32::MAX));
        for (x, y) in [
            (-1, 0),
            (0, -1),
            (i64::from(u32::MAX) + 1, 0),
            (0, i64::MAX),
        ] {
            assert_eq!(
                SourcePixel::try_from_signed(x, y).unwrap_err().code,
                ErrorCode::InputSchema
            );
        }
        assert_eq!(SourcePixel::new(u32::MAX, 0).x(), u32::MAX);
    }
}
