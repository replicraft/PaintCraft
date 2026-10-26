//! ICNS inventory keeps logical dimensions and density distinct from pixel size.
use crate::{
    i18n::import_loss_description,
    icon_export::{Cache, Frame},
    t,
};

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Entry {
    pub index: u16,
    pub record: u16,
    pub tag: String,
    pub logical_width: u32,
    pub logical_height: u32,
    pub scale: u32,
    pub width: u32,
    pub height: u32,
    pub encoding: &'static str,
    pub qualified: bool,
}
impl Entry {
    pub fn eligible(&self, pixel_limit: u64) -> bool {
        self.qualified
            && self.width > 0
            && self.height > 0
            && self.width <= 1024
            && self.height <= 1024
            && u64::from(self.width) * u64::from(self.height) <= pixel_limit
    }
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Response {
    Pending,
    Cancel,
    Preview(u16),
    Import(u16),
}
#[derive(Debug)]
pub struct State<P> {
    pub selected: Option<u16>,
    pub accepted: bool,
    prepared_index: Option<u16>,
    cache: Cache<u16, P>,
    error: Option<String>,
}
impl<P> Default for State<P> {
    fn default() -> Self {
        Self {
            selected: None,
            accepted: false,
            prepared_index: None,
            cache: Cache::default(),
            error: None,
        }
    }
}
impl<P> State<P> {
    pub fn value(&self) -> Option<&P> {
        (self.selected == self.prepared_index)
            .then(|| self.cache.value())
            .flatten()
    }
    pub fn prepare(
        &mut self,
        ctx: &egui::Context,
        index: u16,
        prepare: impl FnOnce() -> Result<P, String>,
    ) -> bool {
        if self.selected != Some(index) {
            return false;
        }
        self.accepted = false;
        self.error = None;
        self.cache = Cache::default();
        let mut error = None;
        self.cache.update(ctx, index, || {
            prepare().inspect_err(|message| error = Some(message.clone()))
        });
        self.prepared_index = Some(index);
        self.error = error;
        self.cache.value().is_some()
    }
    pub fn dialog(
        &mut self,
        ctx: &egui::Context,
        name: &str,
        entries: &[Entry],
        pixel_limit: u64,
        frame: impl for<'a> Fn(&'a P) -> Frame<'a>,
        losses: impl Fn(&P) -> &[String],
    ) -> Response {
        let mut response = Response::Pending;
        let width = 520.0_f32.min((ctx.viewport_rect().width() - 48.0).max(220.0));
        let popup = egui::Frame::popup(&ctx.style_of(ctx.theme()));
        let id = egui::Id::unique("icns-entry-import");
        let area = egui::Modal::default_area(id)
            .anchor(egui::Align2::CENTER_TOP, egui::vec2(0.0, 24.0))
            .default_width(width + popup.total_margin().sum().x)
            .default_height((ctx.viewport_rect().height() - 48.0).max(160.0));
        egui::Modal::new(id).area(area).frame(popup).show(ctx,|ui|{
            ui.set_width(width);
            ui.heading(t!(ctx,"Choose an Apple icon entry"));
            ui.add(egui::Label::new(name).truncate());
            let mut ready=false;
            egui::ScrollArea::vertical().id_salt("icns-import-content").auto_shrink([false,false]).max_height((ctx.viewport_rect().height()-180.0).max(120.0)).show(ui,|ui|{
                ui.label(t!(ctx,"Import one entry as a picture. Other entries remain in the source file."));
                ui.label(t!(ctx,"Choose one entry explicitly. Pixel size, logical size and display scale are different properties."));
                let before=self.selected;
                for entry in entries {
                    ui.add_enabled_ui(entry.eligible(pixel_limit),|ui|{
                        ui.radio_value(&mut self.selected,Some(entry.index),t!(ctx,"Entry { $number }: { $tag }",number=u32::from(entry.index)+1,tag=entry.tag.as_str()));
                    });
                    ui.label(t!(ctx,"Logical size: { $width } × { $height }, display scale: { $scale }×",width=entry.logical_width,height=entry.logical_height,scale=entry.scale));
                    ui.label(t!(ctx,"Pixel size: { $width } × { $height } px",width=entry.width,height=entry.height));
                    ui.label(t!(ctx,"Record { $number }: { $encoding }",number=u32::from(entry.record)+1,encoding=t!(ctx,entry.encoding)));
                    if !entry.qualified {ui.label(t!(ctx,"This entry's encoding cannot be imported yet."));}
                    else if !entry.eligible(pixel_limit){ui.label(t!(ctx,"This entry exceeds the canvas size limit."));}
                    ui.add_space(8.0);
                }
                if before!=self.selected {self.cache=Cache::default();self.prepared_index=None;self.accepted=false;self.error=None;}
                let current=self.selected.and_then(|index|entries.iter().find(|entry|entry.index==index));
                if ui.add_enabled(current.is_some_and(|e|e.eligible(pixel_limit)),egui::Button::new(t!(ctx,"Update preview"))).clicked() && let Some(index)=self.selected {
                    response=Response::Preview(index);
                }
                if let Some(error)=&self.error{ui.colored_label(ui.visuals().error_fg_color,error);}
                if self.selected==self.prepared_index && self.value().is_some(){
                    let shown=self.cache.show_bounded(ui,0,1024,"Import preview",|p,_|Some(frame(p)));
                    let mut known=true;
                    if let Some(prepared)=self.cache.value(){for loss in losses(prepared){
                        if let Some(text)=import_loss_description(loss){ui.label(t!(ctx,text));}else{known=false;}
                    }}
                    ui.checkbox(&mut self.accepted,t!(ctx,"I accept these import changes."));
                    ready=shown && known && self.accepted && current.is_some_and(|e|e.eligible(pixel_limit));
                }
            });
            ui.separator();
            ui.horizontal(|ui|{
                if ui.button(t!(ctx,"Cancel")).clicked(){response=Response::Cancel;}
                if ui.add_enabled(ready,egui::Button::new(t!(ctx,"Import selected entry"))).clicked() && let Some(index)=self.selected {response=Response::Import(index);}
            });
        });
        if ctx.current_pass_index() != 0 {
            return Response::Pending;
        }
        if ctx.input(|i| i.key_pressed(egui::Key::Escape)) {
            return Response::Cancel;
        }
        response
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn density_is_metadata_and_admission_uses_physical_pixels_without_implicit_choice() {
        let retina = Entry {
            index: 0,
            record: 4,
            tag: "ic11".into(),
            logical_width: 16,
            logical_height: 16,
            scale: 2,
            width: 32,
            height: 32,
            encoding: "PNG",
            qualified: true,
        };
        let one_x = Entry {
            index: 1,
            record: 8,
            tag: "il32".into(),
            logical_width: 32,
            logical_height: 32,
            scale: 1,
            encoding: "RGB and transparency mask",
            ..retina.clone()
        };
        assert_ne!(retina, one_x);
        for entry in [&retina, &one_x] {
            assert!(!entry.eligible(1023));
            assert!(entry.eligible(1024));
        }
        assert!(
            !Entry {
                qualified: false,
                ..retina.clone()
            }
            .eligible(u64::MAX)
        );
        let state: State<Vec<u8>> = State::default();
        assert!(state.selected.is_none());
        assert!(state.value().is_none());
        assert!(!state.accepted);
    }
    #[test]
    fn failed_or_stale_preview_cannot_reuse_approved_pixels() {
        let ctx = egui::Context::default();
        let mut state: State<Vec<u8>> = State {
            selected: Some(2),
            ..State::default()
        };
        assert!(!state.prepare(&ctx, 1, || panic!("stale preview must not decode")));
        assert!(state.prepare(&ctx, 2, || Ok(vec![1, 2, 3, 4])));
        state.accepted = true;
        assert_eq!(state.value().unwrap(), &[1, 2, 3, 4]);
        assert!(!state.prepare(&ctx, 2, || Err("invalid pixels".into())));
        assert!(state.value().is_none());
        assert!(!state.accepted);
        assert!(state.prepare(&ctx, 2, || Ok(vec![5, 6, 7, 8])));
        state.selected = Some(3);
        assert!(state.value().is_none());
    }
}
