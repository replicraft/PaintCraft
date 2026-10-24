//! History rows are presentation data; the host validates and executes selection.

use paintcraft_ui::t;
#[derive(Clone, Debug)]
pub struct HistoryRow<'a> {
    pub label: &'a str,
    pub current: bool,
    pub redo: bool,
}

pub fn history_window(
    ctx: &egui::Context,
    open: &mut bool,
    rows: &[HistoryRow<'_>],
    trimmed: u64,
    enabled: bool,
    top: f32,
) -> Option<usize> {
    let mut selected = None;
    egui::Window::new(t!(ctx, "History")).id(egui::Id::unique("studio-history"))
        .open(open).default_width(240.0).default_height(160.0)
        .default_pos(crate::layout::position(ctx, "studio-history", egui::pos2((ctx.viewport_rect().right()-260.0).max(170.0),top.max(150.0))))
        .constrain_to(egui::Rect::from_min_max(egui::pos2(ctx.viewport_rect().left(),top),ctx.viewport_rect().max))
        .show(ctx,|ui| {
            if trimmed > 0 { ui.label(t!(ui.ctx(), "{ $count } oldest history step(s) were dropped to stay within the memory budget.", &[("count", trimmed.to_string())])); }
            if !enabled { ui.weak(t!(ui.ctx(), "Finish or cancel the active tool before navigating history.")); }
            ui.add_enabled_ui(enabled,|ui| {
                let height = ui.text_style_height(&egui::TextStyle::Body)+6.0;
                egui::ScrollArea::vertical().id_salt("studio-history-rows").stick_to_bottom(true)
                    .show_rows(ui,height,rows.len(),|ui,range| {
                        for i in range {
                            let row = &rows[i];
                            let label = t!(ui.ctx(), row.label);
let title = if row.redo { t!(ui.ctx(), "{ $label } (redo)", &[("label", label)]) } else { label };
                            let response = ui.push_id(i,|ui| ui.selectable_label(row.current,&title)).inner;
                            let accessible = t!(ui.ctx(), "History { $number }: { $title }", &[("number", (i+1).to_string()), ("title", title.clone())]);
response.widget_info(|| egui::WidgetInfo::selected(egui::Role::Button,ui.is_enabled(),row.current,&accessible));
                            if response.clicked() && !row.current { selected = Some(i); }
                        }
                    });
            });
        });
    selected
}
