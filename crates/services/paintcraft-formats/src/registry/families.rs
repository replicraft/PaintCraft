//! Family identifiers serve admission, including recognized but unavailable inputs.
macro_rules! families {
    ($($variant:ident => $id:literal, [$($ext:literal),+];)+) => {
        #[derive(Clone,Copy,Debug,PartialEq,Eq,Hash)]
        pub enum Family { $($variant),+ }
        impl Family {
            pub const ALL:&'static [Self]=&[$(Self::$variant),+];
            pub fn id(self)->&'static str {match self {$(Self::$variant=>$id),+}}
            pub fn extensions(self)->&'static [&'static str] {match self {$(Self::$variant=>&[$($ext),+]),+}}
        }
    }
}
families! {
    Pcnd=>"pcnd",["pcnd"];
    Png=>"png",["png"];
    Jpeg=>"jpeg",["jpg","jpeg","jpe","jfif","exif"];
    Bmp=>"bmp",["bmp","dib","rle"];
    Pdn=>"pdn",["pdn"];
    Paint=>"paint",["paint"];
    Gif=>"gif",["gif"];
    Webp=>"webp",["webp"];
    Tiff=>"tiff",["tif","tiff"];
    Tga=>"tga",["tga"];
    Dds=>"dds",["dds"];
    Avif=>"avif",["avif"];
    Jxl=>"jxl",["jxl"];
    Jxr=>"jxr",["jxr","wdp","wmp"];
    Heif=>"heif",["heic","heif","hif"];
    Ico=>"ico",["ico"];
    Cur=>"cur",["cur"];
    Pcx=>"pcx",["pcx"];
    Ora=>"ora",["ora"];
    Psd=>"psd",["psd"];
    Psb=>"psb",["psb"];
    Svg=>"svg",["svg","svgz"];
    Exr=>"exr",["exr"];
    Hdr=>"hdr",["hdr"];
    Qoi=>"qoi",["qoi"];
    Jp2=>"jp2",["jp2","j2k","j2c"];
    Msp=>"msp",["msp"];
    Dcx=>"dcx",["dcx"];
    Pnm=>"pnm",["pnm","pbm","pgm","ppm","pam"];
    Farbfeld=>"farbfeld",["ff"];
    Icns=>"icns",["icns"];
    Animation=>"animation",["apng"];
    Raw=>"raw",["dng","cr2","nef","arw"];
    Pdf=>"pdf",["pdf"];
    Kra=>"kra",["kra","krz"];
    Xcf=>"xcf",["xcf"];
    Abr=>"abr",["abr"];
    Ase=>"ase",["ase"];
    Csh=>"csh",["csh"];
}
impl Family {
    /// Resolve a codec family identifier independently of its filename aliases.
    pub fn from_id(id: &str) -> Option<Self> {
        Self::ALL
            .iter()
            .copied()
            .find(|family| family.id().eq_ignore_ascii_case(id))
    }
    pub fn from_extension(extension: &str) -> Option<Self> {
        let extension = extension.strip_prefix('.').unwrap_or(extension);
        Self::ALL.iter().copied().find(|family| {
            family
                .extensions()
                .iter()
                .any(|candidate| candidate.eq_ignore_ascii_case(extension))
        })
    }
    /// Known aliases are not automatically qualified container variants.
    pub fn qualified_extension(self, extension: &str) -> bool {
        let extension = extension.strip_prefix('.').unwrap_or(extension);
        match self {
            Self::Pcnd => extension.eq_ignore_ascii_case("pcnd"),
            Self::Png => extension.eq_ignore_ascii_case("png"),
            Self::Jpeg => ["jpg", "jpeg", "jpe", "jfif"]
                .iter()
                .any(|ext| ext.eq_ignore_ascii_case(extension)),
            Self::Bmp => extension.eq_ignore_ascii_case("bmp"),
            Self::Gif => extension.eq_ignore_ascii_case("gif"),
            Self::Jxl => extension.eq_ignore_ascii_case("jxl"),
            Self::Jxr => ["jxr", "wdp", "wmp"]
                .iter()
                .any(|candidate| extension.eq_ignore_ascii_case(candidate)),
            Self::Heif => ["heic", "heif", "hif"]
                .iter()
                .any(|candidate| extension.eq_ignore_ascii_case(candidate)),
            Self::Dds => extension.eq_ignore_ascii_case("dds"),
            Self::Avif => extension.eq_ignore_ascii_case("avif"),
            Self::Pdn => extension.eq_ignore_ascii_case("pdn"),
            Self::Ora => extension.eq_ignore_ascii_case("ora"),
            Self::Psd => extension.eq_ignore_ascii_case("psd"),
            Self::Psb => extension.eq_ignore_ascii_case("psb"),
            Self::Svg => ["svg", "svgz"]
                .iter()
                .any(|ext| ext.eq_ignore_ascii_case(extension)),
            Self::Paint => extension.eq_ignore_ascii_case("paint"),
            Self::Dcx => extension.eq_ignore_ascii_case("dcx"),
            Self::Icns => extension.eq_ignore_ascii_case("icns"),
            Self::Ico => extension.eq_ignore_ascii_case("ico"),
            Self::Cur => extension.eq_ignore_ascii_case("cur"),
            Self::Qoi => extension.eq_ignore_ascii_case("qoi"),
            Self::Farbfeld => extension.eq_ignore_ascii_case("ff"),
            Self::Pcx => extension.eq_ignore_ascii_case("pcx"),
            Self::Msp => extension.eq_ignore_ascii_case("msp"),
            Self::Abr => extension.eq_ignore_ascii_case("abr"),
            Self::Ase => extension.eq_ignore_ascii_case("ase"),
            Self::Csh => extension.eq_ignore_ascii_case("csh"),
            Self::Pnm => ["pnm", "pbm", "pgm", "ppm", "pam"]
                .iter()
                .any(|ext| ext.eq_ignore_ascii_case(extension)),
            Self::Tga => extension.eq_ignore_ascii_case("tga"),
            Self::Webp => extension.eq_ignore_ascii_case("webp"),
            Self::Tiff => ["tif", "tiff"]
                .iter()
                .any(|ext| ext.eq_ignore_ascii_case(extension)),
            _ => false,
        }
    }
    pub fn is_project(self) -> bool {
        matches!(
            self,
            Self::Pcnd
                | Self::Pdn
                | Self::Paint
                | Self::Psd
                | Self::Psb
                | Self::Ora
                | Self::Kra
                | Self::Xcf
        )
    }
    pub fn is_asset(self) -> bool {
        matches!(self, Self::Abr | Self::Ase | Self::Csh)
    }
}
