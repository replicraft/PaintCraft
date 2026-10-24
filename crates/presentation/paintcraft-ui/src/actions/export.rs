//! Portable image-export intentions. Encoders remain owned by the frontend host.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[repr(u8)]
pub enum ImageExport {
    Png = 0,
    Jpeg = 1,
    Bmp = 2,
}
impl ImageExport {
    pub const ALL: [Self; 3] = [Self::Png, Self::Jpeg, Self::Bmp];
    pub fn extension(self) -> &'static str {
        match self {
            Self::Png => "png",
            Self::Jpeg => "jpg",
            Self::Bmp => "bmp",
        }
    }
    pub fn name(self) -> &'static str {
        match self {
            Self::Png => "PNG",
            Self::Jpeg => "JPEG",
            Self::Bmp => "BMP",
        }
    }
    pub fn label(self) -> &'static str {
        match self {
            Self::Png => "PNG picture",
            Self::Jpeg => "JPEG picture",
            Self::Bmp => "BMP picture",
        }
    }
    pub fn description(self) -> &'static str {
        match self {
            Self::Png => "Save a high-quality picture with transparency.",
            Self::Jpeg => "Save a smaller picture for photos. Transparency is not preserved.",
            Self::Bmp => "Save an uncompressed bitmap picture.",
        }
    }
}
