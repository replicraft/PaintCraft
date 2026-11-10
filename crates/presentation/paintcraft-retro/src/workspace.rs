//! One implemented skin and its independently retained presentation state.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Skin {
    #[default]
    Win95,
}
impl Skin {
    pub const ALL: [Self; 1] = [Self::Win95];
    pub const fn id(self) -> &'static str {
        "win95"
    }
    pub const fn name(self) -> &'static str {
        "Win95-inspired"
    }
    pub fn parse(id: &str) -> Option<Self> {
        (id == "win95").then_some(Self::Win95)
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Workspace {
    pub skin: Skin,
    pub toolbox: bool,
    pub colors: bool,
    pub status: bool,
    pub text_toolbar: bool,
    /// Accessible alternate to right-clicking a palette color.
    pub edit_secondary: bool,
}
impl Workspace {
    pub fn new(skin: Skin) -> Self {
        Self {
            skin,
            toolbox: true,
            colors: true,
            status: true,
            text_toolbar: true,
            edit_secondary: false,
        }
    }
}
impl Default for Workspace {
    fn default() -> Self {
        Self::new(Skin::Win95)
    }
}
