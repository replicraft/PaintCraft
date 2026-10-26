//! Fresh ICNS output uses explicit qualified types, never inferred Retina sizes.
use crate::{icon_export::Filter, t};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Kind {
    Size128,
    Size256,
    Size512,
    Retina16,
    Retina32,
    Retina128,
    Retina256,
    Retina512,
}
impl Kind {
    pub const ALL: [Self; 8] = [
        Self::Size128,
        Self::Size256,
        Self::Size512,
        Self::Retina16,
        Self::Retina32,
        Self::Retina128,
        Self::Retina256,
        Self::Retina512,
    ];
    pub fn tag(self) -> &'static str {
        match self {
            Self::Size128 => "ic07",
            Self::Size256 => "ic08",
            Self::Size512 => "ic09",
            Self::Retina16 => "ic11",
            Self::Retina32 => "ic12",
            Self::Retina128 => "ic13",
            Self::Retina256 => "ic14",
            Self::Retina512 => "ic10",
        }
    }
    pub fn logical_size(self) -> u32 {
        match self {
            Self::Retina16 => 16,
            Self::Retina32 => 32,
            Self::Size128 | Self::Retina128 => 128,
            Self::Size256 | Self::Retina256 => 256,
            Self::Size512 | Self::Retina512 => 512,
        }
    }
    pub fn scale(self) -> u32 {
        match self {
            Self::Size128 | Self::Size256 | Self::Size512 => 1,
            _ => 2,
        }
    }
    pub fn pixel_size(self) -> u32 {
        self.logical_size() * self.scale()
    }
}
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Config {
    pub kinds: Vec<Kind>,
    pub filter: Filter,
}
impl Default for Config {
    fn default() -> Self {
        Self {
            kinds: Vec::new(),
            filter: Filter::Lanczos3,
        }
    }
}
impl Config {
    pub fn valid(&self) -> bool {
        !self.kinds.is_empty()
            && self.kinds.len() <= Kind::ALL.len()
            && !self
                .kinds
                .iter()
                .enumerate()
                .any(|(i, kind)| self.kinds[..i].contains(kind))
            && self
                .kinds
                .iter()
                .map(|kind| u64::from(kind.pixel_size()).pow(2))
                .sum::<u64>()
                <= 4_194_304
    }
}
#[derive(Debug, Default)]
pub struct Draft {
    pub config: Config,
    pub selected: usize,
    pub accepted: bool,
}
pub fn controls(ui: &mut egui::Ui, draft: &mut Draft) {
    let ctx = ui.ctx().clone();
    let before = draft.config.clone();
    ui.label(t!(
        &ctx,
        "Choose each icon type explicitly. Equal pixel sizes can have different display scales."
    ));
    ui.label(t!(
        &ctx,
        "Each entry is fitted independently, preserving aspect ratio on transparent padding."
    ));
    for kind in Kind::ALL {
        let mut selected = draft.config.kinds.contains(&kind);
        if ui.checkbox(&mut selected,t!(&ctx,"{ $tag }: { $logical } × { $logical } at { $scale }×, { $pixels } × { $pixels } px",tag=kind.tag(),logical=kind.logical_size(),scale=kind.scale(),pixels=kind.pixel_size())).changed(){
            if selected {draft.config.kinds.push(kind);}else{draft.config.kinds.retain(|value|*value!=kind);}
        }
    }
    egui::ComboBox::from_label(t!(&ctx, "Resampling"))
        .selected_text(t!(
            &ctx,
            match draft.config.filter {
                Filter::Nearest => "Nearest neighbor",
                Filter::Lanczos3 => "Lanczos3",
            }
        ))
        .show_ui(ui, |ui| {
            for (value, label) in [
                (Filter::Nearest, "Nearest neighbor"),
                (Filter::Lanczos3, "Lanczos3"),
            ] {
                ui.selectable_value(&mut draft.config.filter, value, t!(&ctx, label));
            }
        });
    if !draft.config.valid() {
        ui.label(t!(&ctx, "Choose at least one supported icon type."));
    }
    draft.selected = draft
        .selected
        .min(draft.config.kinds.len().saturating_sub(1));
    if let Some(kind) = draft.config.kinds.get(draft.selected) {
        egui::ComboBox::from_label(t!(&ctx, "Preview entry"))
            .selected_text(kind.tag())
            .show_ui(ui, |ui| {
                for (index, kind) in draft.config.kinds.iter().enumerate() {
                    ui.selectable_value(&mut draft.selected, index, kind.tag());
                }
            });
    }
    if before != draft.config {
        draft.accepted = false;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn fresh_output_has_only_explicit_qualified_types_and_no_automatic_retina_choice() {
        assert!(!Config::default().valid());
        let tags: Vec<_> = Kind::ALL.iter().map(|kind| kind.tag()).collect();
        assert_eq!(
            tags,
            [
                "ic07", "ic08", "ic09", "ic11", "ic12", "ic13", "ic14", "ic10"
            ]
        );
        assert_eq!(
            (
                Kind::Size128.logical_size(),
                Kind::Size128.scale(),
                Kind::Size128.pixel_size()
            ),
            (128, 1, 128)
        );
        assert_eq!(
            (
                Kind::Retina16.logical_size(),
                Kind::Retina16.scale(),
                Kind::Retina16.pixel_size()
            ),
            (16, 2, 32)
        );
        assert_eq!(
            (
                Kind::Retina512.logical_size(),
                Kind::Retina512.scale(),
                Kind::Retina512.pixel_size()
            ),
            (512, 2, 1024)
        );
        assert!(
            Config {
                kinds: Kind::ALL.to_vec(),
                ..Config::default()
            }
            .valid()
        );
        assert!(
            !Config {
                kinds: vec![Kind::Retina16; 2],
                ..Config::default()
            }
            .valid()
        );
    }
}
