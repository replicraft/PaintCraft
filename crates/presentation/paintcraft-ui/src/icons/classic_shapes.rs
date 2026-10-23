//! Classic uses tintable outline geometry; the colored icon sheet stays separate.
use super::Icon;

pub(super) fn source(icon: Icon) -> Option<&'static [u8]> {
    Some(match icon {
        Icon::Line => include_bytes!("../../assets/icons/classic-shapes/line.svg"),
        Icon::Curve => include_bytes!("../../assets/icons/classic-shapes/curve.svg"),
        Icon::Ellipse => include_bytes!("../../assets/icons/classic-shapes/ellipse.svg"),
        Icon::Rectangle => include_bytes!("../../assets/icons/classic-shapes/rectangle.svg"),
        Icon::RoundedRectangle => {
            include_bytes!("../../assets/icons/classic-shapes/rounded-rectangle.svg")
        }
        Icon::Polygon => include_bytes!("../../assets/icons/classic-shapes/polygon.svg"),
        Icon::Triangle => include_bytes!("../../assets/icons/classic-shapes/triangle.svg"),
        Icon::RightTriangle => {
            include_bytes!("../../assets/icons/classic-shapes/right-triangle.svg")
        }
        Icon::Diamond => include_bytes!("../../assets/icons/classic-shapes/diamond.svg"),
        Icon::Pentagon => include_bytes!("../../assets/icons/classic-shapes/pentagon.svg"),
        Icon::Hexagon => include_bytes!("../../assets/icons/classic-shapes/hexagon.svg"),
        Icon::RightArrow => include_bytes!("../../assets/icons/classic-shapes/right-arrow.svg"),
        Icon::LeftArrow => include_bytes!("../../assets/icons/classic-shapes/left-arrow.svg"),
        Icon::UpArrow => include_bytes!("../../assets/icons/classic-shapes/up-arrow.svg"),
        Icon::DownArrow => include_bytes!("../../assets/icons/classic-shapes/down-arrow.svg"),
        Icon::FourPointStar => {
            include_bytes!("../../assets/icons/classic-shapes/four-point-star.svg")
        }
        Icon::Star => include_bytes!("../../assets/icons/classic-shapes/star.svg"),
        Icon::SixPointStar => {
            include_bytes!("../../assets/icons/classic-shapes/six-point-star.svg")
        }
        Icon::RoundedSpeechBubble => {
            include_bytes!("../../assets/icons/classic-shapes/rounded-speech-bubble.svg")
        }
        Icon::SpeechBubble => include_bytes!("../../assets/icons/classic-shapes/speech-bubble.svg"),
        Icon::CloudCallout => include_bytes!("../../assets/icons/classic-shapes/cloud-callout.svg"),
        Icon::Heart => include_bytes!("../../assets/icons/classic-shapes/heart.svg"),
        Icon::Lightning => include_bytes!("../../assets/icons/classic-shapes/lightning.svg"),
        _ => return None,
    })
}
