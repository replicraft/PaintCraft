//! Session-only color sampling choices; these do not mutate document pixels.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SampleSource {
    Image,
    Layer,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SampleSize {
    One,
    Three,
    Five,
    Eleven,
    ThirtyOne,
    FiftyOne,
}
impl SampleSize {
    pub const ALL: [Self; 6] = [
        Self::One,
        Self::Three,
        Self::Five,
        Self::Eleven,
        Self::ThirtyOne,
        Self::FiftyOne,
    ];
    pub const fn side(self) -> u32 {
        match self {
            Self::One => 1,
            Self::Three => 3,
            Self::Five => 5,
            Self::Eleven => 11,
            Self::ThirtyOne => 31,
            Self::FiftyOne => 51,
        }
    }
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PickerAfter {
    Stay,
    Previous,
    Pencil,
}
