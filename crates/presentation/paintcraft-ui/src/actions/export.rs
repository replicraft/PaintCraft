//! Portable image-export intentions. Encoders remain owned by the frontend host.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[repr(u8)]
pub enum ImageExport {
    Png = 0,
    Jpeg = 1,
    Bmp = 2,
    Gif = 3,
    Tiff = 4,
    Tga = 5,
    Webp = 6,
    Jxl = 7,
    Dds = 8,
    Avif = 9,
    Qoi = 10,
    Pam = 11,
    Farbfeld = 12,
}
impl ImageExport {
    pub const ALL: [Self; 13] = [
        Self::Png,
        Self::Jpeg,
        Self::Bmp,
        Self::Gif,
        Self::Tiff,
        Self::Tga,
        Self::Webp,
        Self::Jxl,
        Self::Dds,
        Self::Avif,
        Self::Qoi,
        Self::Pam,
        Self::Farbfeld,
    ];
    /// Presentation inventory only. The host must still perform format admission.
    pub fn available(self, frontend: super::Frontend) -> bool {
        frontend == super::Frontend::Studio
            || !matches!(
                self,
                Self::Tga
                    | Self::Webp
                    | Self::Jxl
                    | Self::Dds
                    | Self::Avif
                    | Self::Qoi
                    | Self::Pam
                    | Self::Farbfeld
            )
    }
    pub fn extension(self) -> &'static str {
        match self {
            Self::Png => "png",
            Self::Jpeg => "jpg",
            Self::Bmp => "bmp",
            Self::Gif => "gif",
            Self::Tiff => "tiff",
            Self::Tga => "tga",
            Self::Webp => "webp",
            Self::Jxl => "jxl",
            Self::Dds => "dds",
            Self::Avif => "avif",
            Self::Qoi => "qoi",
            Self::Pam => "pam",
            Self::Farbfeld => "ff",
        }
    }
    pub fn name(self) -> &'static str {
        match self {
            Self::Png => "PNG",
            Self::Jpeg => "JPEG",
            Self::Bmp => "BMP",
            Self::Gif => "GIF",
            Self::Tiff => "TIFF",
            Self::Tga => "TGA",
            Self::Webp => "WebP",
            Self::Jxl => "JPEG XL",
            Self::Dds => "DDS",
            Self::Avif => "AVIF",
            Self::Qoi => "QOI",
            Self::Pam => "PAM",
            Self::Farbfeld => "farbfeld",
        }
    }
    pub fn label(self) -> &'static str {
        match self {
            Self::Png => "PNG picture",
            Self::Jpeg => "JPEG picture",
            Self::Bmp => "BMP picture",
            Self::Gif => "GIF picture",
            Self::Tiff => "TIFF picture",
            Self::Tga => "TGA picture",
            Self::Webp => "WebP picture",
            Self::Jxl => "JPEG XL picture",
            Self::Dds => "DDS picture",
            Self::Avif => "AVIF picture",
            Self::Qoi => "QOI picture",
            Self::Pam => "PAM picture",
            Self::Farbfeld => "farbfeld picture",
        }
    }
    pub fn description(self) -> &'static str {
        match self {
            Self::Png => "Save a high-quality picture with transparency.",
            Self::Jpeg => "Save a smaller picture for photos. Transparency is not preserved.",
            Self::Bmp => "Save an uncompressed bitmap picture.",
            Self::Gif => "Save a palette picture. Colors and transparency may be reduced.",
            Self::Tiff => "Save a single-page TIFF picture with transparency.",
            Self::Tga => "Save a TGA picture with transparency.",
            Self::Webp => "Save a lossless WebP picture with transparency.",
            Self::Jxl => {
                "Save a lossless 8-bit JPEG XL picture. Requires at least two pixels per side."
            }
            Self::Dds => {
                "Save an uncompressed, single-surface sRGB DDS picture with straight alpha."
            }
            Self::Avif => "Save a lossy 8-bit AVIF picture. Alpha may change, even at quality 100.",
            Self::Qoi => "Save a lossless QOI picture with transparency.",
            Self::Pam => "Save a PAM Netpbm picture with transparency.",
            Self::Farbfeld => {
                "Save a farbfeld picture with exact 8-bit colors stored as 16-bit samples."
            }
        }
    }
    pub fn quality_label(self) -> Option<&'static str> {
        match self {
            Self::Jpeg => Some("JPEG quality"),
            Self::Avif => Some("AVIF quality"),
            _ => None,
        }
    }
    pub fn mime(self) -> &'static str {
        match self {
            Self::Png => "image/png",
            Self::Jpeg => "image/jpeg",
            Self::Bmp => "image/bmp",
            Self::Gif => "image/gif",
            Self::Tiff => "image/tiff",
            Self::Tga => "image/x-tga",
            Self::Webp => "image/webp",
            Self::Jxl => "image/jxl",
            Self::Dds => "image/vnd-ms.dds",
            Self::Avif => "image/avif",
            Self::Qoi => "image/qoi",
            Self::Pam => "image/x-portable-arbitrarymap",
            Self::Farbfeld => "image/x-farbfeld",
        }
    }
}
