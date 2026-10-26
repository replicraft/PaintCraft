//! Explicit raster-placement preview and conversion consent; hosts own sources.
use crate::t;
use paintcraft_core::types::{Rgba, policy::COORD_ABS_MAX};
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Config {
    pub at: (i64, i64),
    pub width: u32,
    pub height: u32,
    pub color: Rgba,
}
#[derive(Clone, Copy, Debug)]
pub struct Limits {
    pub side: u32,
    pub pixels: u64,
}
impl Config {
    pub fn valid(self, limits: Limits) -> bool {
        self.width > 0
            && self.height > 0
            && self.width <= limits.side
            && self.height <= limits.side
            && u64::from(self.width) * u64::from(self.height) <= limits.pixels
            && [
                Some(self.at.0),
                Some(self.at.1),
                self.at.0.checked_add(self.width.into()),
                self.at.1.checked_add(self.height.into()),
            ]
            .iter()
            .all(|value| value.is_some_and(|v| v.unsigned_abs() <= COORD_ABS_MAX as u64))
    }
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Response {
    Pending,
    Cancel,
    Preview(Config),
    Place,
}
pub struct State<P> {
    pub config: Config,
    pub accepted: bool,
    texture: Option<egui::TextureHandle>,
    prepared: Option<(Config, P)>,
    shown: Option<u64>,
    error: Option<String>,
}
impl<P> std::fmt::Debug for State<P> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ShapePlacementState")
            .field("config", &self.config)
            .field("accepted", &self.accepted)
            .finish_non_exhaustive()
    }
}
impl<P> State<P> {
    pub fn new(config: Config) -> Self {
        Self {
            config,
            accepted: false,
            texture: None,
            prepared: None,
            shown: None,
            error: None,
        }
    }
    pub fn clear(&mut self) {
        self.texture = None;
        self.prepared = None;
        self.accepted = false;
        self.shown = None;
        self.error = None;
    }
    pub fn fail(&mut self, error: String) {
        self.clear();
        self.error = Some(error);
    }
    pub fn install(
        &mut self,
        ctx: &egui::Context,
        config: Config,
        prepared: P,
        bytes: impl FnOnce(&P) -> &[u8],
    ) -> bool {
        let rgba = bytes(&prepared);
        if config.width == 0
            || config.height == 0
            || config != self.config
            || u64::from(config.width)
                .checked_mul(u64::from(config.height))
                .and_then(|bytes| bytes.checked_mul(4))
                .and_then(|bytes| usize::try_from(bytes).ok())
                != Some(rgba.len())
        {
            return false;
        }
        self.clear();
        self.texture = Some(ctx.load_texture(
            "csh-placement-preview",
            egui::ColorImage::from_rgba_unmultiplied(
                [config.width as usize, config.height as usize],
                rgba,
            ),
            egui::TextureOptions::NEAREST,
        ));
        self.prepared = Some((config, prepared));
        true
    }
    pub fn ready(&self, ctx: &egui::Context, limits: Limits) -> bool {
        self.config.valid(limits)
            && self.accepted
            && self
                .prepared
                .as_ref()
                .is_some_and(|(config, _)| *config == self.config)
            && self.texture.is_some()
            && self
                .shown
                .is_some_and(|frame| frame < ctx.cumulative_frame_nr())
    }
    pub fn take(&mut self, ctx: &egui::Context, limits: Limits) -> Option<P> {
        if !self.ready(ctx, limits) {
            return None;
        }
        let prepared = self.prepared.take().map(|(_, p)| p);
        self.clear();
        prepared
    }
    /// Separate adoption consent applies to future drag boxes, not numeric values.
    pub fn tool_dialog(&mut self, ctx: &egui::Context, name: &str, limits: Limits) -> Response {
        let mut response = Response::Pending;
        let width = 480.0_f32.min((ctx.viewport_rect().width() - 48.0).max(220.0));
        let frame = egui::Frame::popup(&ctx.style_of(ctx.theme()));
        let id = egui::Id::unique("csh-tool-options");
        let area = egui::Modal::default_area(id)
            .anchor(egui::Align2::CENTER_TOP, egui::vec2(0.0, 24.0))
            .default_width(width + frame.total_margin().sum().x)
            .default_height((ctx.viewport_rect().height() - 48.0).max(160.0));
        egui::Modal::new(id).area(area).frame(frame).show(ctx, |ui| {
            ui.set_width(480.0_f32.min((ctx.viewport_rect().width()-48.0).max(220.0)));
            ui.heading(t!(ctx,"Use custom shape tool"));
            ui.add(egui::Label::new(name).truncate());
            egui::ScrollArea::vertical().id_salt("shape-tool-content").auto_shrink([false,false]).max_height((ctx.viewport_rect().height()-180.0).max(100.0)).show(ui,|ui|{
                ui.label(t!(ctx,"Drag on the canvas to set the shape size. Hold Shift for a square box."));
                ui.label(t!(ctx,"Each shape becomes filled pixels in the current color. Its original resource stays unchanged."));
                ui.label(t!(ctx,"Curves may look slightly different from the original shape."));
                ui.label(t!(ctx,"Canvas edges and the current selection clip the placed shape."));
                ui.label(t!(ctx,"This preview shows the shape at a sample size; drawing uses your drag box."));
                if ui.button(t!(ctx,"Update preview")).clicked(){self.clear();response=Response::Preview(self.config);}
                if let Some(error)=&self.error{ui.colored_label(ui.visuals().error_fg_color,error);}
                if let Some(texture)=&self.texture {
                    ui.label(t!(ctx,"Shape preview"));
                    let (rect,_)=ui.allocate_exact_size(texture.size_vec2(),egui::Sense::hover());
                    ui.painter().rect_filled(rect,0.0,egui::Color32::from_gray(210));
                    ui.painter().image(texture.id(),rect,egui::Rect::from_min_max(egui::Pos2::ZERO,egui::pos2(1.0,1.0)),egui::Color32::WHITE);
                    if self.shown.is_none() && ui.is_rect_visible(rect){self.shown=Some(ctx.cumulative_frame_nr());ctx.request_repaint();}
                    ui.checkbox(&mut self.accepted,t!(ctx,"I accept converting each drawn shape to pixels."));
                }
            });
            ui.separator();
            ui.horizontal(|ui| {
                if ui.button(t!(ctx,"Cancel")).clicked(){response=Response::Cancel;}
                if ui.add_enabled(self.ready(ctx,limits),egui::Button::new(t!(ctx,"Use as shape tool"))).clicked(){response=Response::Place;}
            });
        });
        if ctx.input(|i| i.key_pressed(egui::Key::Escape)) {
            response = Response::Cancel;
        }
        response
    }
    pub fn dialog(&mut self, ctx: &egui::Context, name: &str, limits: Limits) -> Response {
        let mut response = Response::Pending;
        egui::Modal::new(egui::Id::unique("csh-placement-options")).show(ctx,|ui|{
            ui.set_width(480.0_f32.min((ctx.viewport_rect().width()-48.0).max(220.0)));ui.heading(t!(ctx,"Place custom shape"));ui.add(egui::Label::new(name).truncate());
            egui::ScrollArea::vertical().id_salt("shape-placement-content").max_height((ctx.viewport_rect().height()-180.0).max(100.0)).show(ui,|ui|{
                let before=self.config;
                ui.horizontal_wrapped(|ui|{
                    for (label,value) in [("X",&mut self.config.at.0),("Y",&mut self.config.at.1)]{let text=t!(ctx,label);let response=ui.label(&text);ui.add(egui::DragValue::new(value).range(-COORD_ABS_MAX..=COORD_ABS_MAX).clamp_existing_to_range(false)).labelled_by(response.id).widget_info(||egui::WidgetInfo::labeled(egui::Role::SpinButton,ui.is_enabled(),&text));}
                });
                ui.horizontal_wrapped(|ui|{
                    for (label,value) in [("Width",&mut self.config.width),("Height",&mut self.config.height)]{let text=t!(ctx,label);let response=ui.label(&text);ui.add(egui::DragValue::new(value).range(1..=limits.side).clamp_existing_to_range(false)).labelled_by(response.id).widget_info(||egui::WidgetInfo::labeled(egui::Role::SpinButton,ui.is_enabled(),&text));}
                });
                ui.horizontal(|ui|{ui.label(t!(ctx,"Fill color"));let mut color=self.config.color.0;if ui.color_edit_button_srgba_unmultiplied(&mut color).changed(){self.config.color=Rgba(color);}});
                if before!=self.config{self.clear();}
                ui.label(t!(ctx,"This shape becomes pixels at the chosen size. Its original vector resource stays unchanged."));
                ui.label(t!(ctx,"Curves may look slightly different from the original shape."));
                ui.label(t!(ctx,"Canvas edges and the current selection clip the placed shape."));
                if !self.config.valid(limits){ui.colored_label(ui.visuals().error_fg_color,t!(ctx,"Choose a valid position and size within the placement limits."));}
                if ui.add_enabled(self.config.valid(limits),egui::Button::new(t!(ctx,"Update preview"))).clicked(){self.clear();response=Response::Preview(self.config);}
                if let Some(error)=&self.error{ui.colored_label(ui.visuals().error_fg_color,error);}
                if let Some(texture)=&self.texture{
                    ui.label(t!(ctx,"Shape preview"));let size=texture.size_vec2();let scale=(200.0/size.x.max(size.y)).min(1.0);let(rect,_)=ui.allocate_exact_size(size*scale,egui::Sense::hover());
                    for y in 0..(rect.height()/8.0).ceil() as usize{for x in 0..(rect.width()/8.0).ceil() as usize{let cell=egui::Rect::from_min_size(rect.min+egui::vec2(x as f32*8.0,y as f32*8.0),egui::vec2(8.0,8.0)).intersect(rect);ui.painter().rect_filled(cell,0.0,egui::Color32::from_gray(if(x+y)%2==0{160}else{210}));}}
                    ui.painter().image(texture.id(),rect,egui::Rect::from_min_max(egui::Pos2::ZERO,egui::pos2(1.0,1.0)),egui::Color32::WHITE);
                    if self.shown.is_none()&&ui.is_rect_visible(rect){self.shown=Some(ctx.cumulative_frame_nr());ctx.request_repaint();}
                    ui.checkbox(&mut self.accepted,t!(ctx,"I accept converting this shape to pixels."));
                }
            });ui.separator();ui.horizontal(|ui|{if ui.button(t!(ctx,"Cancel")).clicked(){response=Response::Cancel;}
                if ui.add_enabled(self.ready(ctx,limits),egui::Button::new(t!(ctx,"Place shape"))).clicked(){response=Response::Place;}});
        });
        if ctx.input(|i| i.key_pressed(egui::Key::Escape)) {
            response = Response::Cancel;
        }
        response
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn invalid_dimensions_and_signed_coordinate_overflow_never_install_or_enable_placement() {
        let limits = Limits {
            side: 4096,
            pixels: 4_194_304,
        };
        let color = Rgba([200, 40, 10, 128]);
        let valid = Config {
            at: (-8, 0),
            width: 8,
            height: 4,
            color,
        };
        assert!(valid.valid(limits));
        for config in [
            Config { width: 0, ..valid },
            Config { height: 0, ..valid },
            Config {
                width: u32::MAX,
                height: u32::MAX,
                ..valid
            },
            Config {
                at: (COORD_ABS_MAX, 0),
                ..valid
            },
            Config {
                at: (i64::MIN, 0),
                ..valid
            },
        ] {
            assert!(!config.valid(limits));
            let ctx = egui::Context::default();
            let mut state = State::new(config);
            state.accepted = true;
            assert!(!state.install(&ctx, config, (), |_| &[]));
            assert!(!state.ready(&ctx, limits));
            assert!(state.take(&ctx, limits).is_none());
        }
    }
    #[test]
    fn unchanged_color_widget_does_not_quantize_translucent_paint_or_enable_unreviewed_preview() {
        let ctx = egui::Context::default();
        let config = Config {
            at: (0, 0),
            width: 4,
            height: 4,
            color: Rgba([200, 40, 10, 128]),
        };
        let mut state = State::<()>::new(config);
        let limits = Limits {
            side: 4096,
            pixels: 4_194_304,
        };
        for _ in 0..3 {
            let mut output = ctx.run_ui(Default::default(), |ui| {
                state.dialog(ui.ctx(), "Shape", limits);
            });
            output.textures_delta.clear();
            assert_eq!(state.config, config);
            assert!(!state.ready(&ctx, limits));
        }
    }
}
