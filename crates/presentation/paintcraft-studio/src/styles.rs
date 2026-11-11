//! Ordered style intentions; only the native owner queues captured transactions.
use egui::{Context, Ui};
use paintcraft_core::types::styles::{ColorOverlayRgbV1, EffectBlendMode, StyleDescriptor};
use paintcraft_ui::t;
use std::{fmt::Debug, hash::Hash};

#[derive(Clone, Copy, Debug)]
pub struct Row<Id> {
    pub id: Id,
    pub enabled: bool,
    pub descriptor: StyleDescriptor,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Intent<Id> {
    Add,
    Edit(Id),
    SetEnabled { id: Id, enabled: bool },
    Move { id: Id, to_index: usize },
    Duplicate(Id),
    Remove(Id),
}
pub fn panel<Id: Copy + Hash + Debug>(
    ui: &mut Ui,
    rows: &[Row<Id>],
    can_add: bool,
    enabled: bool,
) -> Vec<Intent<Id>> {
    let mut actions = Vec::new();
    ui.separator();
    ui.heading(t!(ui.ctx(), "Layer styles"));
    ui.add_enabled_ui(enabled, |ui| {
        if ui
            .add_enabled(
                can_add,
                egui::Button::new(t!(ui.ctx(), "Add Color Overlay")),
            )
            .clicked()
        {
            actions.push(Intent::Add);
        }
        for (index, row) in rows.iter().enumerate() {
            ui.push_id(row.id, |ui| {
                let mut value = row.enabled;
                if ui
                    .checkbox(&mut value, t!(ui.ctx(), "Color Overlay"))
                    .changed()
                {
                    actions.push(Intent::SetEnabled {
                        id: row.id,
                        enabled: value,
                    });
                }
                ui.horizontal_wrapped(|ui| {
                    if ui.button(t!(ui.ctx(), "Edit Color Overlay")).clicked() {
                        actions.push(Intent::Edit(row.id));
                    }
                    if ui
                        .add_enabled(index > 0, egui::Button::new(t!(ui.ctx(), "Move up")))
                        .clicked()
                    {
                        actions.push(Intent::Move {
                            id: row.id,
                            to_index: index - 1,
                        });
                    }
                    if ui
                        .add_enabled(
                            index + 1 < rows.len(),
                            egui::Button::new(t!(ui.ctx(), "Move down")),
                        )
                        .clicked()
                    {
                        actions.push(Intent::Move {
                            id: row.id,
                            to_index: index + 1,
                        });
                    }
                    if ui
                        .add_enabled(can_add, egui::Button::new(t!(ui.ctx(), "Duplicate")))
                        .clicked()
                    {
                        actions.push(Intent::Duplicate(row.id));
                    }
                    if ui.button(t!(ui.ctx(), "Remove")).clicked() {
                        actions.push(Intent::Remove(row.id));
                    }
                });
            });
        }
    });
    actions
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Parameters {
    pub rgb: [u8; 3],
    pub opacity: u8,
    pub blend: EffectBlendMode,
    pub preview: bool,
}
impl Parameters {
    pub fn new(rgb: [u8; 3]) -> Self {
        Self {
            rgb,
            opacity: 255,
            blend: EffectBlendMode::Normal,
            preview: true,
        }
    }
    pub fn from_descriptor(value: StyleDescriptor) -> Self {
        match value {
            StyleDescriptor::ColorOverlayRgbV1(value) => Self {
                rgb: value.color(),
                opacity: value.opacity(),
                blend: value.blend(),
                preview: true,
            },
        }
    }
    pub fn descriptor(self) -> StyleDescriptor {
        StyleDescriptor::ColorOverlayRgbV1(ColorOverlayRgbV1::new(
            self.rgb,
            self.opacity,
            self.blend,
        ))
    }
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Response {
    Pending,
    Preview(Option<StyleDescriptor>),
    Apply(StyleDescriptor),
    Cancel,
}
pub fn parameter_dialog(ctx: &Context, values: &mut Parameters, style_enabled: bool) -> Response {
    let before = *values;
    let modal = egui::Modal::new(egui::Id::unique("studio-style-parameters")).show(ctx, |ui| {
        ui.set_width(360.0_f32.min((ctx.content_rect().width() - 32.0).max(220.0)));
        ui.heading(t!(ctx, "Color Overlay"));
        ui.label(t!(
            ctx,
            "Color Overlay affects the whole layer. The layer mask applies afterward."
        ));
        ui.horizontal(|ui| {
            let label = ui.label(t!(ctx, "Overlay color"));
            ui.color_edit_button_srgb(&mut values.rgb)
                .labelled_by(label.id);
        });
        ui.horizontal_wrapped(|ui| {
            for (component, caption) in values.rgb.iter_mut().zip(["Red", "Green", "Blue"]) {
                let label = ui.label(t!(ctx, caption));
                ui.add(egui::DragValue::new(component).range(0..=255))
                    .labelled_by(label.id);
            }
        });
        ui.add(
            egui::Slider::new(&mut values.opacity, 0..=255).text(t!(ctx, "Style opacity (0–255)")),
        );
        ui.horizontal(|ui| {
            let label = ui.label(t!(ctx, "Effect blend mode"));
            egui::ComboBox::from_id_salt("style-effect-blend")
                .selected_text(mode_name(ctx, values.blend))
                .show_ui(ui, |ui| {
                    for mode in [
                        EffectBlendMode::Normal,
                        EffectBlendMode::Multiply,
                        EffectBlendMode::Screen,
                    ] {
                        ui.selectable_value(&mut values.blend, mode, mode_name(ctx, mode));
                    }
                })
                .response
                .labelled_by(label.id);
        });
        if !style_enabled {
            ui.label(t!(
                ctx,
                "This style is disabled. Changes are stored without changing its preview."
            ));
        }
        ui.checkbox(&mut values.preview, t!(ctx, "Preview"));
        ui.horizontal(|ui| {
            if ui.button(t!(ctx, "Apply")).clicked() {
                Response::Apply(values.descriptor())
            } else if ui.button(t!(ctx, "Cancel")).clicked() {
                Response::Cancel
            } else {
                Response::Pending
            }
        })
        .inner
    });
    if modal.should_close() {
        return Response::Cancel;
    }
    match modal.inner {
        Response::Pending if before != *values => {
            Response::Preview(values.preview.then(|| values.descriptor()))
        }
        response => response,
    }
}
fn mode_name(ctx: &Context, value: EffectBlendMode) -> String {
    match value {
        EffectBlendMode::Normal => t!(ctx, "Normal"),
        EffectBlendMode::Multiply => t!(ctx, "Multiply"),
        EffectBlendMode::Screen => t!(ctx, "Screen"),
    }
}
