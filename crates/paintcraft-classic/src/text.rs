//! Reference-driven text toolbar prototype, separate from raster text policy.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum TextAlignment {
    #[default]
    Left,
    Center,
    Right,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct TextOptions {
    pub family: String,
    pub points: u16,
    pub bold: bool,
    pub italic: bool,
    pub underline: bool,
    pub strikeout: bool,
    pub alignment: TextAlignment,
    pub background_fill: bool,
    pub editable_until_save: bool,
}

impl Default for TextOptions {
    fn default() -> Self {
        Self {
            family: "Selawik".into(),
            points: 11,
            bold: false,
            italic: false,
            underline: false,
            strikeout: false,
            alignment: TextAlignment::Left,
            background_fill: false,
            editable_until_save: false,
        }
    }
}

pub fn text_options(
    ui: &mut egui::Ui,
    options: &mut TextOptions,
    families: &[String],
    search: &mut String,
    loading: bool,
) {
    egui::Frame::new().corner_radius(8).fill(ui.visuals().widgets.inactive.weak_bg_fill).inner_margin(8).show(ui,|ui| {
        egui::ScrollArea::horizontal().id_salt("classic-text-options").max_height(42.0).show(ui,|ui| {
            ui.horizontal(|ui| {
                let font=egui::ComboBox::from_id_salt("classic-font-family").width(180.0).height(240.0).selected_text(&options.family).show_ui(ui,|ui| {
                    let find=ui.text_edit_singleline(search);
                    find.widget_info(||egui::WidgetInfo::labeled(egui::Role::TextInput,ui.is_enabled(),"Search fonts"));
                    let filter=search.to_lowercase();
                    for name in families.iter().filter(|name|name.to_lowercase().contains(&filter)) {
                        ui.selectable_value(&mut options.family,name.clone(),name);
                    }
                    if loading {ui.weak("Loading installed fonts…");}
                });
                font.response.widget_info(||egui::WidgetInfo::labeled(egui::Role::ComboBox,ui.is_enabled(),"Font family"));
                let size=ui.add(egui::DragValue::new(&mut options.points).range(1..=512).suffix(" pt"));
                size.widget_info(||egui::WidgetInfo::labeled(egui::Role::SpinButton,ui.is_enabled(),"Font size"));
                ui.separator();
                for (value,text,label) in [(&mut options.bold,egui::RichText::new("B").strong(),"Bold"),(&mut options.italic,egui::RichText::new("I").italics(),"Italic"),(&mut options.underline,egui::RichText::new("U").underline(),"Underline"),(&mut options.strikeout,egui::RichText::new("S").strikethrough(),"Strikeout")] {
                    let response=ui.add(egui::Button::new(text).selected(*value).min_size(egui::vec2(28.0,28.0)));
                    response.widget_info(||egui::WidgetInfo::selected(egui::Role::Button,ui.is_enabled(),*value,label));
                    if response.clicked() && ui.ctx().current_pass_index()==0 {*value = !*value;}
                }
                ui.separator();
                for (value,label) in [(TextAlignment::Left,"Align left"),(TextAlignment::Center,"Align center"),(TextAlignment::Right,"Align right")] {
                    let response=ui.add(egui::Button::new("     ").selected(options.alignment==value).min_size(egui::vec2(28.0,28.0)));
                    let center=response.rect.center();
                    for (i,width) in [15.0,10.0,15.0,10.0].into_iter().enumerate() {
                        let x=match value {TextAlignment::Left=>center.x-7.5,TextAlignment::Center=>center.x-width/2.0,TextAlignment::Right=>center.x+7.5-width};
                        let y=center.y-6.0+i as f32*4.0;
                        ui.painter().line_segment([egui::pos2(x,y),egui::pos2(x+width,y)],egui::Stroke::new(1.0,ui.visuals().text_color()));
                    }
                    response.widget_info(||egui::WidgetInfo::selected(egui::Role::Button,ui.is_enabled(),options.alignment==value,label));
                    if response.clicked() {options.alignment=value;}
                }
                ui.separator();
                ui.checkbox(&mut options.background_fill,"Background fill");
                ui.weak("Preview").on_hover_text("Text styling and editable-until-save are UI prototypes. They do not change the current bitmap text renderer.");
            });
        });
    });
}
