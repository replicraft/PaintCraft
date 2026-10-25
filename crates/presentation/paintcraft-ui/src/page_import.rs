//! Shared explicit-page selection; decoding and source ownership stay with hosts.
use crate::t;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Page {
    pub index: u16,
    pub width: u32,
    pub height: u32,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Response {
    Pending,
    Cancel,
    Import(u16),
}

/// No page is selected implicitly, including a one-page source. The host must
/// validate its captured source again before honoring an Import response.
pub fn dialog(
    ctx: &egui::Context,
    name: &str,
    pages: &[Page],
    selected: &mut Option<u16>,
    pixel_limit: u64,
    error: Option<&str>,
) -> Response {
    let response = egui::Modal::new(egui::Id::unique("dcx-page-import")).show(ctx, |ui| {
        ui.set_width(420.0_f32.min((ctx.content_rect().width() - 48.0).max(160.0)));
        ui.heading(t!(ctx, "Choose a page to import"));
        ui.label(name);
        ui.label(t!(ctx, "DCX pages: { $count }", count = pages.len()));
        ui.label(t!(
            ctx,
            "Import one page as a picture. Other pages remain in the source file."
        ));
        egui::ScrollArea::vertical()
            .id_salt("dcx-page-list")
            .max_height(240.0)
            .show(ui, |ui| {
                for page in pages {
                    let label = t!(
                        ctx,
                        "Page { $number }: { $width } × { $height } px",
                        number = u32::from(page.index) + 1,
                        width = page.width,
                        height = page.height
                    );
                    ui.radio_value(selected, Some(page.index), label);
                    ui.add_space(4.0);
                }
            });
        let current = selected.and_then(|index| pages.iter().find(|page| page.index == index));
        let eligible = current
            .is_some_and(|page| u64::from(page.width) * u64::from(page.height) <= pixel_limit);
        if current.is_some() && !eligible {
            ui.label(t!(ctx, "This page exceeds the canvas size limit."));
        }
        if let Some(error) = error {
            ui.colored_label(ui.visuals().error_fg_color, error);
        }
        ui.separator();
        ui.horizontal(|ui| {
            let cancel = ui.button(t!(ctx, "Cancel")).clicked();
            let apply = ui
                .add_enabled(eligible, egui::Button::new(t!(ctx, "Import selected page")))
                .clicked();
            (cancel, apply)
        })
        .inner
    });
    if ctx.current_pass_index() != 0 {
        return Response::Pending;
    }
    if response.inner.0 || response.should_close() {
        Response::Cancel
    } else if response.inner.1 {
        selected.map_or(Response::Pending, Response::Import)
    } else {
        Response::Pending
    }
}
