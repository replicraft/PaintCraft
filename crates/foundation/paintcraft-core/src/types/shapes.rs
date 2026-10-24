//! Stable names for the closed, drag-to-size shape vocabulary.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ShapePreset {
    RoundedRectangle,
    Triangle,
    RightTriangle,
    Diamond,
    Pentagon,
    Hexagon,
    RightArrow,
    LeftArrow,
    UpArrow,
    DownArrow,
    FourPointStar,
    FivePointStar,
    SixPointStar,
    RectangularCallout,
    OvalCallout,
    CloudCallout,
    Heart,
    Lightning,
}

impl ShapePreset {
    pub const ALL: [Self; 18] = [
        Self::RoundedRectangle,
        Self::Triangle,
        Self::RightTriangle,
        Self::Diamond,
        Self::Pentagon,
        Self::Hexagon,
        Self::RightArrow,
        Self::LeftArrow,
        Self::UpArrow,
        Self::DownArrow,
        Self::FourPointStar,
        Self::FivePointStar,
        Self::SixPointStar,
        Self::RectangularCallout,
        Self::OvalCallout,
        Self::CloudCallout,
        Self::Heart,
        Self::Lightning,
    ];

    pub fn command_name(self) -> &'static str {
        match self {
            Self::RoundedRectangle => "rounded-rect",
            Self::Triangle => "triangle",
            Self::RightTriangle => "right-triangle",
            Self::Diamond => "diamond",
            Self::Pentagon => "pentagon",
            Self::Hexagon => "hexagon",
            Self::RightArrow => "right-arrow",
            Self::LeftArrow => "left-arrow",
            Self::UpArrow => "up-arrow",
            Self::DownArrow => "down-arrow",
            Self::FourPointStar => "four-point-star",
            Self::FivePointStar => "five-point-star",
            Self::SixPointStar => "six-point-star",
            Self::RectangularCallout => "rectangular-callout",
            Self::OvalCallout => "oval-callout",
            Self::CloudCallout => "cloud-callout",
            Self::Heart => "heart",
            Self::Lightning => "lightning",
        }
    }

    pub fn from_command_name(name: &str) -> Option<Self> {
        Self::ALL
            .into_iter()
            .find(|shape| shape.command_name() == name)
    }

    pub fn name(self) -> &'static str {
        match self {
            Self::RoundedRectangle => "Rounded rectangle",
            Self::Triangle => "Triangle",
            Self::RightTriangle => "Right triangle",
            Self::Diamond => "Diamond",
            Self::Pentagon => "Pentagon",
            Self::Hexagon => "Hexagon",
            Self::RightArrow => "Right arrow",
            Self::LeftArrow => "Left arrow",
            Self::UpArrow => "Up arrow",
            Self::DownArrow => "Down arrow",
            Self::FourPointStar => "Four-point star",
            Self::FivePointStar => "Five-point star",
            Self::SixPointStar => "Six-point star",
            Self::RectangularCallout => "Rectangular callout",
            Self::OvalCallout => "Oval callout",
            Self::CloudCallout => "Cloud callout",
            Self::Heart => "Heart",
            Self::Lightning => "Lightning",
        }
    }
}
