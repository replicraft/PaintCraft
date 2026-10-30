//! Filter presentation only. Hosts own snapshots, preview admission and publication.
use paintcraft_core::types::{
    Result,
    filters::{BlackWhiteV1, BrightnessContrastV1, Descriptor, PosterizeRgbV1, ThresholdLumaV1},
};
use paintcraft_ui::t;
use std::hash::Hash;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Kind {
    Invert,
    BrightnessContrast,
    BlackWhite,
    Threshold,
    Posterize,
    Solarize,
}
impl Kind {
    pub const ALL: [Self; 6] = [
        Self::Invert,
        Self::BrightnessContrast,
        Self::BlackWhite,
        Self::Threshold,
        Self::Posterize,
        Self::Solarize,
    ];
    pub fn label(self) -> &'static str {
        match self {
            Self::Invert => "Invert",
            Self::BrightnessContrast => "Brightness / Contrast",
            Self::BlackWhite => "Black & White",
            Self::Threshold => "Threshold",
            Self::Posterize => "Posterize",
            Self::Solarize => "Solarize",
        }
    }
    pub fn of(descriptor: Descriptor) -> Self {
        match descriptor {
            Descriptor::InvertRgbV1 => Self::Invert,
            Descriptor::BrightnessContrastV1(_) => Self::BrightnessContrast,
            Descriptor::BlackWhiteRgbV1(_) => Self::BlackWhite,
            Descriptor::ThresholdLumaV1(_) => Self::Threshold,
            Descriptor::PosterizeRgbV1(_) => Self::Posterize,
            Descriptor::SolarizeRgbV1 => Self::Solarize,
        }
    }
}

#[derive(Clone, Copy, Debug)]
pub struct MaskSummary {
    pub enabled: bool,
    pub runs: usize,
}

#[derive(Clone, Copy, Debug)]
pub struct Row<Id> {
    pub id: Id,
    pub enabled: bool,
    pub descriptor: Descriptor,
    pub mask: Option<MaskSummary>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Action<Id> {
    Add(Kind),
    Edit(Id),
    SetEnabled { id: Id, enabled: bool },
    Reorder { id: Id, to_index: usize },
    Duplicate(Id),
    Remove(Id),
    UseSelection(Id),
    SetMaskEnabled { id: Id, enabled: bool },
    RemoveMask(Id),
}

/// Rows are in evaluation order. The host checks stable identities and limits
/// again when it publishes; a displayed row is never editing authority.
pub fn panel<Id: Copy + Hash + std::fmt::Debug>(
    ui: &mut egui::Ui,
    rows: &[Row<Id>],
    can_add: bool,
    selection_active: bool,
) -> Vec<Action<Id>> {
    let mut actions = Vec::new();
    ui.horizontal(|ui| {
        ui.strong(t!(ui.ctx(), "Filters"));
        ui.add_enabled_ui(can_add, |ui| {
            ui.menu_button(t!(ui.ctx(), "Add filter"), |ui| {
                for kind in Kind::ALL {
                    if ui.button(t!(ui.ctx(), kind.label())).clicked() {
                        actions.push(Action::Add(kind));
                        ui.close();
                    }
                }
            });
        });
    });
    if rows.is_empty() {
        ui.weak(t!(ui.ctx(), "No filters on this layer."));
    } else {
        ui.weak(t!(ui.ctx(), "Filters are applied from top to bottom."));
    }
    for (index, row) in rows.iter().enumerate() {
        ui.push_id(row.id, |ui| {
            ui.horizontal(|ui| {
                let mut enabled = row.enabled;
                if ui
                    .checkbox(&mut enabled, t!(ui.ctx(), Kind::of(row.descriptor).label()))
                    .changed()
                {
                    actions.push(Action::SetEnabled {
                        id: row.id,
                        enabled,
                    });
                }
                if ui.small_button(t!(ui.ctx(), "Edit")).clicked() {
                    actions.push(Action::Edit(row.id));
                }
                let menu = ui.menu_button("…", |ui| {
                    if ui
                        .add_enabled(index > 0, egui::Button::new(t!(ui.ctx(), "Move up")))
                        .clicked()
                    {
                        actions.push(Action::Reorder {
                            id: row.id,
                            to_index: index - 1,
                        });
                        ui.close();
                    }
                    if ui
                        .add_enabled(
                            index + 1 < rows.len(),
                            egui::Button::new(t!(ui.ctx(), "Move down")),
                        )
                        .clicked()
                    {
                        actions.push(Action::Reorder {
                            id: row.id,
                            to_index: index + 1,
                        });
                        ui.close();
                    }
                    if ui
                        .add_enabled(can_add, egui::Button::new(t!(ui.ctx(), "Duplicate")))
                        .clicked()
                    {
                        actions.push(Action::Duplicate(row.id));
                        ui.close();
                    }
                    ui.separator();
                    if ui
                        .add_enabled(
                            selection_active,
                            egui::Button::new(t!(ui.ctx(), "Use Selection")),
                        )
                        .clicked()
                    {
                        actions.push(Action::UseSelection(row.id));
                        ui.close();
                    }
                    if let Some(mask) = row.mask {
                        let mut enabled = mask.enabled;
                        if ui
                            .checkbox(&mut enabled, t!(ui.ctx(), "Enable filter mask"))
                            .changed()
                        {
                            actions.push(Action::SetMaskEnabled {
                                id: row.id,
                                enabled,
                            });
                            ui.close();
                        }
                        if ui.button(t!(ui.ctx(), "Remove mask")).clicked() {
                            actions.push(Action::RemoveMask(row.id));
                            ui.close();
                        }
                    }
                    if ui.button(t!(ui.ctx(), "Remove filter")).clicked() {
                        actions.push(Action::Remove(row.id));
                        ui.close();
                    }
                });
                let title = t!(ui.ctx(), "Filter actions");
                menu.response.widget_info(|| {
                    egui::WidgetInfo::labeled(egui::Role::Button, ui.is_enabled(), &title)
                });
                let _ = menu.response.on_hover_text(title);
            });
            mask_summary(ui, row.mask);
        });
    }
    if ui.ctx().current_pass_index() != 0 {
        actions.clear();
    }
    actions
}

/// Private UI values, deliberately independent of document or instance ownership.
/// The host stores its captured owner, base and generation alongside this draft.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Parameters {
    pub kind: Kind,
    pub brightness: i16,
    pub contrast: i16,
    pub strength: i16,
    pub threshold: i16,
    pub levels: i16,
    pub preview: bool,
}
impl Parameters {
    pub fn new(kind: Kind) -> Self {
        Self {
            kind,
            brightness: 0,
            contrast: 0,
            strength: 100,
            threshold: 128,
            levels: 6,
            preview: true,
        }
    }
    pub fn from_descriptor(descriptor: Descriptor) -> Self {
        let mut draft = Self::new(Kind::of(descriptor));
        match descriptor {
            Descriptor::BrightnessContrastV1(p) => {
                draft.brightness = p.brightness();
                draft.contrast = p.contrast();
            }
            Descriptor::BlackWhiteRgbV1(p) => draft.strength = i16::from(p.strength()),
            Descriptor::ThresholdLumaV1(p) => draft.threshold = i16::from(p.level()),
            Descriptor::PosterizeRgbV1(p) => draft.levels = i16::from(p.levels()),
            Descriptor::InvertRgbV1 | Descriptor::SolarizeRgbV1 => {}
        }
        draft
    }
    pub fn descriptor(self) -> Result<Descriptor> {
        match self.kind {
            Kind::Invert => Ok(Descriptor::InvertRgbV1),
            Kind::BrightnessContrast => BrightnessContrastV1::new(self.brightness, self.contrast)
                .map(Descriptor::BrightnessContrastV1),
            Kind::BlackWhite => BlackWhiteV1::new(self.strength).map(Descriptor::BlackWhiteRgbV1),
            Kind::Threshold => {
                ThresholdLumaV1::new(self.threshold).map(Descriptor::ThresholdLumaV1)
            }
            Kind::Posterize => PosterizeRgbV1::new(self.levels).map(Descriptor::PosterizeRgbV1),
            Kind::Solarize => Ok(Descriptor::SolarizeRgbV1),
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ParameterResponse {
    Pending,
    /// None retires the private preview; it never removes a committed filter.
    Preview(Option<Descriptor>),
    Apply(Descriptor),
    Cancel,
}

/// Opening the draft requires an initial preview submission by the host. This
/// function emits subsequent changes once per input frame, including removal
/// of the preview when its checkbox is cleared.
pub fn parameter_dialog(
    ctx: &egui::Context,
    values: &mut Parameters,
    mask: Option<MaskSummary>,
) -> ParameterResponse {
    let before = *values;
    let mut staged = before;
    let response = draw_parameters(ctx, &mut staged, mask);
    if ctx.current_pass_index() != 0 {
        return ParameterResponse::Pending;
    }
    *values = staged;
    if response == ParameterResponse::Pending && before != staged {
        ParameterResponse::Preview(if staged.preview {
            staged.descriptor().ok()
        } else {
            None
        })
    } else {
        response
    }
}

fn draw_parameters(
    ctx: &egui::Context,
    values: &mut Parameters,
    mask: Option<MaskSummary>,
) -> ParameterResponse {
    let mut response = ParameterResponse::Pending;
    let mut open = true;
    egui::Window::new(t!(ctx, values.kind.label()))
        .id(egui::Id::unique("studio-filter-parameters"))
        .open(&mut open)
        .collapsible(false)
        .resizable(false)
        .default_width(340.0)
        .show(ctx, |ui| {
            ui.set_min_width(300.0);
            if values.kind == Kind::BrightnessContrast {
                ui.add(
                    egui::Slider::new(&mut values.brightness, -100..=100)
                        .text(t!(ui.ctx(), "Brightness")),
                );
                ui.add(
                    egui::Slider::new(&mut values.contrast, -99..=99)
                        .text(t!(ui.ctx(), "Contrast")),
                );
            }
            match values.kind {
                Kind::BlackWhite => {
                    ui.add(
                        egui::Slider::new(&mut values.strength, 0..=100)
                            .text(t!(ui.ctx(), "Strength (%)")),
                    );
                }
                Kind::Threshold => {
                    ui.add(
                        egui::Slider::new(&mut values.threshold, 0..=255)
                            .text(t!(ui.ctx(), "Threshold")),
                    );
                }
                Kind::Posterize => {
                    ui.add(
                        egui::Slider::new(&mut values.levels, 2..=32).text(t!(ui.ctx(), "Levels")),
                    );
                }
                Kind::Invert | Kind::BrightnessContrast | Kind::Solarize => {}
            }
            ui.checkbox(&mut values.preview, t!(ui.ctx(), "Preview"));
            mask_summary(ui, mask);
            ui.weak(t!(ui.ctx(), "Source pixels remain unchanged."));
            ui.separator();
            let descriptor = values.descriptor().ok();
            ui.horizontal(|ui| {
                if ui
                    .add_enabled(
                        descriptor.is_some(),
                        egui::Button::new(t!(ui.ctx(), "Apply")),
                    )
                    .clicked()
                    && let Some(descriptor) = descriptor
                {
                    response = ParameterResponse::Apply(descriptor);
                }
                if ui.button(t!(ui.ctx(), "Cancel")).clicked() {
                    response = ParameterResponse::Cancel;
                }
            });
        });
    if !open || ctx.input(|input| input.key_pressed(egui::Key::Escape)) {
        return ParameterResponse::Cancel;
    }
    response
}

/// Small presentation facts only; mask payloads never enter the UI model.
pub fn mask_summary(ui: &mut egui::Ui, mask: Option<MaskSummary>) {
    if let Some(mask) = mask {
        ui.small(t!(
            ui.ctx(),
            if !mask.enabled {
                "Mask disabled: whole layer affected."
            } else if mask.runs == 0 {
                "Empty mask: no pixels affected."
            } else {
                "Filter uses the saved selection."
            }
        ));
    } else {
        ui.weak(t!(ui.ctx(), "Applies to the whole layer."));
    }
}

#[cfg(test)]
#[path = "filters/tests.rs"]
mod tests;
