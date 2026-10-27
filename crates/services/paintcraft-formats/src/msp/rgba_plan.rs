//! Exact two-stage RGBA admission without a speculative full packed allocation.
use super::*;
/// Maximum packed row for positive u16 width. Fixed stack payload, not RSS.
const ROW_BYTES: usize = 8192;
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct RgbaLayout {
    version: Version,
    width: u32,
    height: u32,
    rgba_bytes: usize,
    packed_bytes: usize,
}
impl RgbaLayout {
    pub fn new(version: Version, width: u32, height: u32) -> Result<Self> {
        let (_, row) = geometry(width, height)?;
        Ok(Self {
            version,
            width,
            height,
            rgba_bytes: width as usize * height as usize * 4,
            packed_bytes: row * height as usize,
        })
    }
    pub fn version(self) -> Version {
        self.version
    }
    pub fn width(self) -> u32 {
        self.width
    }
    pub fn height(self) -> u32 {
        self.height
    }
    pub fn rgba_bytes(self) -> usize {
        self.rgba_bytes
    }
    pub fn packed_bytes(self) -> usize {
        self.packed_bytes
    }
    /// First-stage live payload: one composite plus fixed counting scratch.
    /// Retained source document and facade metadata must be priced separately.
    pub fn render_bytes(self) -> usize {
        self.rgba_bytes + ROW_BYTES
    }
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct RgbaWritePlan {
    layout: RgbaLayout,
    encoded_bytes: usize,
}
impl RgbaWritePlan {
    pub fn layout(self) -> RgbaLayout {
        self.layout
    }
    pub fn encoded_bytes(self) -> usize {
        self.encoded_bytes
    }
    /// Additional payload after render admission: packed pixels + exact output.
    /// The first-stage reservation must remain live alongside this one.
    pub fn additional_bytes(self) -> usize {
        self.layout.packed_bytes + self.encoded_bytes
    }
    /// Revalidate before any full packed/output allocation. A plan for different
    /// samples with a different packet extent cannot bypass its admission quote.
    pub fn encode(self, rgba: &[u8], check: &dyn Fn() -> Result<()>) -> Result<Vec<u8>> {
        let actual = preflight_rgba_exact(self.layout, rgba, check)?;
        if actual != self {
            return Err(bad("MSP samples changed their admitted encoded extent"));
        }
        let bytes = match self.layout.version {
            Version::UncompressedV1 => {
                encode_v1_rgba_exact(self.layout.width, self.layout.height, rgba, check)?
            }
            Version::RowRleV2 => {
                encode_v2_rgba_exact(self.layout.width, self.layout.height, rgba, check)?
            }
        };
        if bytes.len() != self.encoded_bytes {
            return Err(Error::new(
                ErrorCode::InvariantViolation,
                "MSP output disagrees with admitted RGBA plan",
            ));
        }
        check()?;
        Ok(bytes)
    }
}
/// Both versions validate every exact opaque black/white sample. V2 counts using
/// the original packet walker and a fixed row, never a second RLE implementation.
/// Invoke only after admitting layout.render_bytes() (composite + row scratch).
pub fn preflight_rgba_exact(
    layout: RgbaLayout,
    rgba: &[u8],
    check: &dyn Fn() -> Result<()>,
) -> Result<RgbaWritePlan> {
    check()?;
    if rgba.len() != layout.rgba_bytes {
        return Err(bad("MSP RGBA extent disagrees with admitted layout"));
    }
    let row_bytes = (layout.width as usize).div_ceil(8);
    let mut row = [0; ROW_BYTES];
    let mut encoded_bytes = match layout.version {
        Version::UncompressedV1 => 32 + layout.packed_bytes,
        Version::RowRleV2 => 32 + layout.height as usize * 2,
    };
    for pixels in rgba.chunks_exact(layout.width as usize * 4) {
        check()?;
        super::encode::pack_row_rgba_exact(pixels, &mut row[..row_bytes], check)?;
        if layout.version == Version::RowRleV2 {
            let size = super::encode_v2::packets(&row[..row_bytes], check, |_| {})?;
            if size == 0 || size > u16::MAX as usize {
                return Err(limit("MSP2 encoded row exceeds nonzero u16 map"));
            }
            encoded_bytes = encoded_bytes
                .checked_add(size)
                .filter(|n| *n <= BYTES_MAX)
                .ok_or_else(|| limit("MSP2 encoded output allowance"))?;
        }
    }
    if encoded_bytes > BYTES_MAX {
        return Err(limit("MSP encoded output allowance"));
    }
    check()?;
    Ok(RgbaWritePlan {
        layout,
        encoded_bytes,
    })
}
