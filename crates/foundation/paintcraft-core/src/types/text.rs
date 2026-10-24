//! Portable text style descriptors and resolved grayscale coverage runs.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub enum TextAlignment {
    #[default]
    Left,
    Center,
    Right,
}

#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct TextOptions {
    pub family: String,
    pub points: u16,
    pub bold: bool,
    pub italic: bool,
    pub underline: bool,
    pub strikeout: bool,
    pub alignment: TextAlignment,
    pub background_fill: bool,
    pub editable_until_save: bool,
}

impl Default for TextOptions {
    fn default() -> Self {
        Self {
            family: "Selawik".into(),
            points: 11,
            bold: false,
            italic: false,
            underline: false,
            strikeout: false,
            alignment: TextAlignment::Left,
            background_fill: false,
            editable_until_save: false,
        }
    }
}

/// A resolved grayscale mask run in document coordinates; x1 is inclusive.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct MaskSpan {
    pub y: i64,
    pub x0: i64,
    pub x1: i64,
    pub coverage: u8,
}

/// Resolved text coverage and layout geometry. These immutable samples are
/// portable; font lookup and shaping remain outside document transactions.
#[derive(Debug)]
pub struct TextMask {
    pub spans: Vec<MaskSpan>,
    pub bounds: Option<super::Rect>,
    pub caret: (i64, i64, i64),
}
