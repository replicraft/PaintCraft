//! Document tabs emit intentions; session and I/O ownership belong to the host.
use paintcraft_ui::i18n::{tr, tr_format};
#[derive(Clone, Debug)]
pub struct DocumentTab<'a> {
    pub label: &'a str,
    pub number: u64,
    pub dirty: bool,
    pub active: bool,
}

#[derive(Clone, Copy, Debug)]
pub enum TabAction {
    Select(usize),
    Close(usize),
    New,
}

pub fn document_tabs(ui: &mut egui::Ui, tabs: &[DocumentTab<'_>]) -> Option<TabAction> {
    let mut action = None;
    egui::ScrollArea::horizontal()
        .id_salt("studio-document-tabs")
        .show(ui, |ui| {
            ui.horizontal(|ui| {
                for (i, tab) in tabs.iter().enumerate() {
                    ui.push_id(tab.number, |ui| {
                        let label = format!("{}{}", tab.label, if tab.dirty { " *" } else { "" });
                        let r = ui.selectable_label(tab.active, &label);
                        let accessible = tr_format(
                            ui.ctx(),
                            "Document { $number }: { $title }",
                            &[("number", tab.number.to_string()), ("title", label.clone())],
                        );
                        r.widget_info(|| {
                            egui::WidgetInfo::selected(
                                egui::Role::Button,
                                ui.is_enabled(),
                                tab.active,
                                &accessible,
                            )
                        });
                        if r.clicked() {
                            action = Some(TabAction::Select(i));
                        }
                        let r = ui
                            .small_button("×")
                            .on_hover_text(tr(ui.ctx(), "Close document"));
                        let accessible = tr_format(
                            ui.ctx(),
                            "Close document { $number }",
                            &[("number", tab.number.to_string())],
                        );
                        r.widget_info(|| {
                            egui::WidgetInfo::labeled(
                                egui::Role::Button,
                                ui.is_enabled(),
                                &accessible,
                            )
                        });
                        if r.clicked() {
                            action = Some(TabAction::Close(i));
                        }
                    });
                    ui.separator();
                }
                let r = ui
                    .small_button("+")
                    .on_hover_text(tr(ui.ctx(), "New document"));
                let accessible = tr(ui.ctx(), "New document tab");
                r.widget_info(|| {
                    egui::WidgetInfo::labeled(egui::Role::Button, ui.is_enabled(), &accessible)
                });
                if r.clicked() {
                    action = Some(TabAction::New);
                }
            });
        });
    action
}
