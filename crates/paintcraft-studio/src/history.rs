//! History rows are presentation data; the host validates and executes selection.

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
) -> Option<usize> {
    let mut selected = None;
    egui::Window::new("History").id(egui::Id::unique("studio-history"))
        .open(open).default_width(240.0).default_height(160.0)
        .default_pos(egui::pos2((ctx.viewport_rect().right()-260.0).max(170.0),150.0))
        .show(ctx,|ui| {
            if trimmed > 0 { ui.label(format!("{trimmed} oldest history step(s) were dropped to stay within the memory budget.")); }
            if !enabled { ui.weak("Finish or cancel the active tool before navigating history."); }
            ui.add_enabled_ui(enabled,|ui| {
                let height = ui.text_style_height(&egui::TextStyle::Body)+6.0;
                egui::ScrollArea::vertical().id_salt("studio-history-rows").stick_to_bottom(true)
                    .show_rows(ui,height,rows.len(),|ui,range| {
                        for i in range {
                            let row = &rows[i];
                            let title = format!("{}{}",row.label,if row.redo {" (redo)"} else {""});
                            let response = ui.push_id(i,|ui| ui.selectable_label(row.current,&title)).inner;
                            response.widget_info(|| egui::WidgetInfo::selected(egui::Role::Button,ui.is_enabled(),row.current,format!("History {}: {title}",i+1)));
                            if response.clicked() && !row.current { selected = Some(i); }
                        }
                    });
            });
        });
    selected
}
