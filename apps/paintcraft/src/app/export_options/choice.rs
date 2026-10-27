//! Native-only encoder choices wrap portable inventory without adding Web codecs.
use super::*;
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum Choice {
    Flat(ImageExport),
    Jxr,
}
impl Choice {
    pub(super) fn label(self) -> &'static str {
        match self {
            Self::Flat(format) => format.label(),
            Self::Jxr => "JPEG XR picture",
        }
    }
    pub(super) fn description(self) -> &'static str {
        match self {
            Self::Flat(format) => format.description(),
            Self::Jxr => "Save a lossless JPEG XR picture. The picture must be entirely opaque.",
        }
    }
    pub(super) fn extension(self) -> &'static str {
        match self {
            Self::Flat(format) => format.extension(),
            Self::Jxr => "jxr",
        }
    }
    pub(super) fn quality_label(self) -> Option<&'static str> {
        match self {
            Self::Flat(format) => format.quality_label(),
            Self::Jxr => None,
        }
    }
}
