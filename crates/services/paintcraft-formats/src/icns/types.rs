/// Explicit ICNS PNG element types. Equal physical pixel sizes do not select
/// the same logical icon or density, and no automatic type inference occurs.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum IconType {
    Size16,
    Size32,
    Size64,
    Size128,
    Size256,
    Size512,
    Retina16,
    Retina32,
    Retina128,
    Retina256,
    Retina512,
}
impl IconType {
    pub const ALL: [Self; 11] = [
        Self::Size16,
        Self::Size32,
        Self::Size64,
        Self::Size128,
        Self::Size256,
        Self::Size512,
        Self::Retina16,
        Self::Retina32,
        Self::Retina128,
        Self::Retina256,
        Self::Retina512,
    ];
    /// Standard PNG types with exact typed readback through Apple iconutil on
    /// the qualification host. Small1xPNG types have separate reader evidence.
    pub const WRITABLE: [Self; 8] = [
        Self::Size128,
        Self::Size256,
        Self::Size512,
        Self::Retina16,
        Self::Retina32,
        Self::Retina128,
        Self::Retina256,
        Self::Retina512,
    ];
    pub fn writable(self) -> bool {
        Self::WRITABLE.contains(&self)
    }
    pub fn tag(self) -> [u8; 4] {
        *match self {
            Self::Size16 => b"icp4",
            Self::Size32 => b"icp5",
            Self::Size64 => b"icp6",
            Self::Size128 => b"ic07",
            Self::Size256 => b"ic08",
            Self::Size512 => b"ic09",
            Self::Retina16 => b"ic11",
            Self::Retina32 => b"ic12",
            Self::Retina128 => b"ic13",
            Self::Retina256 => b"ic14",
            Self::Retina512 => b"ic10",
        }
    }
    pub fn from_tag(tag: [u8; 4]) -> Option<Self> {
        Self::ALL.into_iter().find(|kind| kind.tag() == tag)
    }
    pub fn logical_size(self) -> u32 {
        match self {
            Self::Size16 | Self::Retina16 => 16,
            Self::Size32 | Self::Retina32 => 32,
            Self::Size64 => 64,
            Self::Size128 | Self::Retina128 => 128,
            Self::Size256 | Self::Retina256 => 256,
            Self::Size512 | Self::Retina512 => 512,
        }
    }
    pub fn scale(self) -> u32 {
        match self {
            Self::Size16
            | Self::Size32
            | Self::Size64
            | Self::Size128
            | Self::Size256
            | Self::Size512 => 1,
            _ => 2,
        }
    }
    pub fn pixel_size(self) -> u32 {
        self.logical_size() * self.scale()
    }
}
