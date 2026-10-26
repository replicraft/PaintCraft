//! Static brush resource controls. Hosts retain source identity and validate adoption.
use crate::t;
#[derive(Clone, Debug)]
pub struct Preset {
    pub name: String,
    pub dimensions: Option<(u32, u32)>,
    pub qualified: bool,
    pub quantized_spacing: bool,
    pub restrictions: Vec<&'static str>,
}
#[derive(Clone, Debug)]
pub struct Resource {
    pub name: String,
    pub presets: Vec<Preset>,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Response {
    Pending,
    Close,
    Import,
    Select(usize),
    Adopt,
    Stock,
}
#[derive(Debug, Default)]
pub struct State {
    pub consent: bool,
}
impl State {
    pub fn reset(&mut self) {
        self.consent = false;
    }
    pub fn dialog(
        &mut self,
        ctx: &egui::Context,
        resource: Option<&Resource>,
        selected: usize,
        busy: bool,
        error: Option<&str>,
        adopted: Option<&str>,
    ) -> Response {
        let mut response = Response::Pending;
        egui::Modal::new(egui::Id::unique("studio-brush-resources")).show(ctx, |ui| {
            ui.set_width(520.0_f32.min((ctx.viewport_rect().width()-48.0).max(220.0)));
            ui.heading(t!(ctx,"Brush resources"));
            egui::ScrollArea::vertical().max_height((ctx.viewport_rect().height()-170.0).max(120.0)).show(ui,|ui|{
                ui.label(t!(ctx,"Import Adobe brushes and choose a supported static tip. Your painting changes only when you draw."));
                if ui.add_enabled(!busy,egui::Button::new(t!(ctx,"Import brushes…"))).clicked(){response=Response::Import;}
                if busy {ui.spinner();}
                if let Some(error)=error {ui.colored_label(ui.visuals().error_fg_color,error);}
                if let Some(adopted)=adopted {ui.label(t!(ctx,"Selected brush: { $name }",name=adopted));}
                if let Some(resource)=resource {
                    ui.add(egui::Label::new(&resource.name).truncate());
                    egui::ScrollArea::vertical().id_salt("brush-preset-list").max_height(140.0).show_rows(ui,24.0,resource.presets.len(),|ui,range|{
                        for index in range {if ui.add_enabled(!busy,egui::Button::selectable(index==selected,&resource.presets[index].name)).clicked(){response=Response::Select(index);}}
                    });
                    if let Some(preset)=resource.presets.get(selected){
                        if let Some((width,height))=preset.dimensions {ui.label(t!(ctx,"Tip size: { $width } × { $height } px",width=width,height=height));}
                        ui.label(t!(ctx,"Static tips paint at their original size. Brush size and Photoshop dynamics are not applied."));
                        for restriction in &preset.restrictions {ui.label(t!(ctx,*restriction));}
                        if preset.qualified {
                            if preset.quantized_spacing {ui.checkbox(&mut self.consent,t!(ctx,"I accept the small change to brush spacing."));}
                            if ui.add_enabled(!busy&&(!preset.quantized_spacing||self.consent),egui::Button::new(t!(ctx,"Use this brush"))).clicked(){response=Response::Adopt;}
                        }else{ui.label(t!(ctx,"This preset cannot be used as a static brush."));}
                    }
                }
                if ui.add_enabled(!busy&&adopted.is_some(),egui::Button::new(t!(ctx,"Use standard brushes"))).clicked(){response=Response::Stock;}
            });
            ui.separator();
            if ui.add_enabled(!busy,egui::Button::new(t!(ctx,"Close"))).clicked(){response=Response::Close;}
        });
        if !busy && ctx.input(|i| i.key_pressed(egui::Key::Escape)) {
            response = Response::Close;
        }
        response
    }
}
