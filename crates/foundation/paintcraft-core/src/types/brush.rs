//! Portable brush media identifiers, independent of presentation and storage.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub enum BrushMedia {
    #[default]
    Plain,
    CalligraphyBrush,
    CalligraphyPen,
    Airbrush,
    Oil,
    Crayon,
    Marker,
    NaturalPencil,
    Watercolor,
}
impl BrushMedia {
    pub const ALL: [Self; 9] = [
        Self::Plain,
        Self::CalligraphyBrush,
        Self::CalligraphyPen,
        Self::Airbrush,
        Self::Oil,
        Self::Crayon,
        Self::Marker,
        Self::NaturalPencil,
        Self::Watercolor,
    ];
    pub fn id(self) -> &'static str {
        match self {
            Self::Plain => "smooth-round",
            Self::CalligraphyBrush => "calligraphy-brush",
            Self::CalligraphyPen => "calligraphy-pen",
            Self::Airbrush => "airbrush",
            Self::Oil => "oil",
            Self::Crayon => "crayon",
            Self::Marker => "marker",
            Self::NaturalPencil => "natural-pencil",
            Self::Watercolor => "watercolor",
        }
    }
    pub fn name(self) -> &'static str {
        match self {
            Self::Plain => "Brush",
            Self::CalligraphyBrush => "Calligraphy brush",
            Self::CalligraphyPen => "Calligraphy pen",
            Self::Airbrush => "Airbrush",
            Self::Oil => "Oil brush",
            Self::Crayon => "Crayon",
            Self::Marker => "Marker",
            Self::NaturalPencil => "Natural pencil",
            Self::Watercolor => "Watercolor brush",
        }
    }
    pub fn parse(id: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|media| media.id() == id)
    }
}
