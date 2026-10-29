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

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Format {
    Dcx,
    Tiff,
}
impl Format {
    fn modal_id(self) -> &'static str {
        match self {
            Self::Dcx => "dcx-page-import",
            Self::Tiff => "tiff-page-import",
        }
    }
    fn list_id(self) -> &'static str {
        match self {
            Self::Dcx => "dcx-page-list",
            Self::Tiff => "tiff-page-list",
        }
    }
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct PageChoice {
    pub page: Page,
    /// Source format/profile qualification, independent of the canvas budget.
    pub supported: bool,
}
/// DCX compatibility entry point. No page is selected implicitly, including a
/// one-page source. Hosts revalidate captured ownership before accepting Import.
pub fn dialog(
    ctx: &egui::Context,
    name: &str,
    pages: &[Page],
    selected: &mut Option<u16>,
    pixel_limit: u64,
    error: Option<&str>,
) -> Response {
    dialog_rows(
        ctx,
        Format::Dcx,
        name,
        pages.iter().copied().map(|page| PageChoice {
            page,
            supported: true,
        }),
        selected,
        pixel_limit,
        error,
    )
}
/// Explicit format/profile-aware selection. Unsupported pages remain visible
/// but cannot become an import request or disable a different qualified page.
pub fn dialog_profiled(
    ctx: &egui::Context,
    format: Format,
    name: &str,
    pages: &[PageChoice],
    selected: &mut Option<u16>,
    pixel_limit: u64,
    error: Option<&str>,
) -> Response {
    dialog_rows(
        ctx,
        format,
        name,
        pages.iter().copied(),
        selected,
        pixel_limit,
        error,
    )
}
fn dialog_rows(
    ctx: &egui::Context,
    format: Format,
    name: &str,
    pages: impl Iterator<Item = PageChoice> + Clone,
    selected: &mut Option<u16>,
    pixel_limit: u64,
    error: Option<&str>,
) -> Response {
    let response = egui::Modal::new(egui::Id::unique(format.modal_id())).show(ctx, |ui| {
        ui.set_width(420.0_f32.min((ctx.content_rect().width() - 48.0).max(160.0)));
        ui.heading(t!(ctx, "Choose a page to import"));
        ui.label(name);
        let count = pages.clone().count();
        ui.label(match format {
            Format::Dcx => t!(ctx, "DCX pages: { $count }", count = count),
            Format::Tiff => t!(ctx, "TIFF pages: { $count }", count = count),
        });
        ui.label(t!(
            ctx,
            "Import one page as a picture. Other pages remain in the source file."
        ));
        egui::ScrollArea::vertical()
            .id_salt(format.list_id())
            .max_height(240.0)
            .show(ui, |ui| {
                for choice in pages.clone() {
                    let page = choice.page;
                    let label = t!(
                        ctx,
                        "Page { $number }: { $width } × { $height } px",
                        number = u32::from(page.index) + 1,
                        width = page.width,
                        height = page.height
                    );
                    ui.add_enabled_ui(choice.supported, |ui| {
                        ui.radio_value(selected, Some(page.index), label);
                    });
                    if !choice.supported {
                        ui.weak(match format {
                            Format::Tiff => t!(ctx, "This page uses an unsupported TIFF profile."),
                            Format::Dcx => t!(ctx, "This page cannot be imported."),
                        });
                    }
                    ui.add_space(4.0);
                }
            });
        let current =
            selected.and_then(|index| pages.clone().find(|choice| choice.page.index == index));
        let eligible = current.is_some_and(|choice| {
            choice.supported
                && u64::from(choice.page.width) * u64::from(choice.page.height) <= pixel_limit
        });
        if current.is_some_and(|choice| choice.supported) && !eligible {
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

#[cfg(test)]
#[path = "page_import/tests.rs"]
mod tests;
