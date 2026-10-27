//! Stable policy descriptors and validation bounds shared by parsers and engines.
pub const POLICY: &str = "paintcraft.core/default-v1";
pub const CANVAS_SIDE_MAX: u32 = 1_048_576;
pub const COORD_ABS_MAX: i64 = 140_737_488_355_327;
pub const LAYER_COUNT_MAX: usize = 100_000;
pub const STAGED_PIXELS_MAX: u64 = 268_435_456;
pub const TEXT_CHARS_MAX: usize = 1024;
pub const SCALE_MAX: i64 = 16;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum BlendMode {
    Replace,
    SourceOver,
}

/// Stable hexadecimal representation used by content digests.
pub fn hex(bytes: &[u8]) -> String {
    use std::fmt::Write as _;
    bytes
        .iter()
        .fold(String::with_capacity(bytes.len() * 2), |mut s, b| {
            write!(s, "{b:02x}").expect("writing to a String");
            s
        })
}

/// Resolved 8-bit coverage policies. Generic paint defaults remain unchanged.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum MaskMode {
    SourceOver,
    Replace,
    /// Attenuate destination alpha; retain its straight RGB even at alpha zero.
    DestinationOut,
}
