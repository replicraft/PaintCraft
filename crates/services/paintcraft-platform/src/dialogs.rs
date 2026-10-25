//! Parented native dialogs. Completion is polled by the application event loop.
use raw_window_handle::{HasDisplayHandle, HasWindowHandle};
use std::{
    path::{Path, PathBuf},
    sync::mpsc,
};

/// Show an error even when creating the parent application window failed.
pub fn show_error(title: &str, description: &str, ok: &str) {
    let _ = rfd::MessageDialog::new()
        .set_title(title)
        .set_description(description)
        .set_level(rfd::MessageLevel::Error)
        .set_buttons(rfd::MessageButtons::OkCustom(ok.to_owned()))
        .show();
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Kind {
    Open,
    InspectRecovery,
    SaveAs,
    Export,
}
#[derive(Debug)]
pub struct Completion {
    pub kind: Kind,
    pub path: Option<PathBuf>,
}
/// The host supplies its qualified frontend inventory; this crate owns only OS UI.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Filter {
    pub name: String,
    pub extensions: Vec<String>,
}
#[derive(Debug)]
pub struct NativeDialogs {
    builder: rfd::AsyncFileDialog,
    sender: mpsc::Sender<Completion>,
    receiver: mpsc::Receiver<Completion>,
    pending: bool,
}
impl NativeDialogs {
    pub fn new(parent: &(impl HasWindowHandle + HasDisplayHandle)) -> Self {
        let (sender, receiver) = mpsc::channel();
        Self {
            builder: rfd::AsyncFileDialog::new().set_parent(parent),
            sender,
            receiver,
            pending: false,
        }
    }
    pub fn pending(&self) -> bool {
        self.pending
    }
    pub fn request(
        &mut self,
        kind: Kind,
        suggested: Option<&Path>,
        wake: impl FnOnce() + Send + 'static,
    ) {
        self.request_with_labels(kind, suggested, str::to_owned, wake);
    }
    /// The host supplies localized labels without coupling OS adapters to UI state.
    pub fn request_with_labels(
        &mut self,
        kind: Kind,
        suggested: Option<&Path>,
        label: impl Fn(&str) -> String,
        wake: impl FnOnce() + Send + 'static,
    ) {
        let filters = match kind {
            Kind::Open => vec![Filter {
                name: "Images and PaintCraft documents".into(),
                extensions: ["pcnd", "png", "bmp", "jpg", "jpeg"]
                    .map(str::to_owned)
                    .into(),
            }],
            Kind::InspectRecovery | Kind::SaveAs => vec![Filter {
                name: "PaintCraft document".into(),
                extensions: vec!["pcnd".into()],
            }],
            Kind::Export => export_filters(suggested)
                .into_iter()
                .map(|(name, extensions)| Filter {
                    name: name.into(),
                    extensions: extensions.iter().map(|ext| (*ext).into()).collect(),
                })
                .collect(),
        };
        self.request_with_filters(kind, suggested, &filters, label, wake);
    }
    pub fn request_with_filters(
        &mut self,
        kind: Kind,
        suggested: Option<&Path>,
        filters: &[Filter],
        label: impl Fn(&str) -> String,
        wake: impl FnOnce() + Send + 'static,
    ) {
        if self.pending {
            return;
        }
        if filters.is_empty()
            || filters.iter().any(|filter| {
                filter.extensions.is_empty()
                    || filter.extensions.iter().any(|ext| {
                        ext.is_empty() || !ext.bytes().all(|byte| byte.is_ascii_alphanumeric())
                    })
            })
        {
            return;
        }
        let mut filters = filters.to_vec();
        if kind == Kind::Export
            && let Some(extension) = suggested
                .and_then(Path::extension)
                .and_then(|ext| ext.to_str())
            && let Some(index) = filters.iter().position(|filter| {
                filter
                    .extensions
                    .iter()
                    .any(|ext| ext.eq_ignore_ascii_case(extension))
            })
        {
            filters.swap(0, index);
        }
        let mut dialog = self.builder.clone();
        if let Some(path) = suggested {
            if let Some(parent) = path.parent().filter(|p| !p.as_os_str().is_empty()) {
                dialog = dialog.set_directory(parent);
            }
            if let Some(name) = path.file_name() {
                dialog = dialog.set_file_name(name.to_string_lossy());
            }
        }
        let fallback_extension = filters[0].extensions[0].clone();
        dialog = match kind {
            Kind::Open => dialog.set_title(label("Open image or PaintCraft document")),
            Kind::InspectRecovery => {
                dialog.set_title(label("Locate the destination of an incomplete save"))
            }
            Kind::SaveAs => dialog.set_title(label("Save PaintCraft document")),
            Kind::Export => dialog.set_title(label("Export image")),
        };
        for filter in filters {
            dialog = dialog.add_filter(label(&filter.name), &filter.extensions);
        }
        self.pending = true;
        let sender = self.sender.clone();
        std::thread::spawn(move || {
            let file = pollster::block_on(async {
                if matches!(kind, Kind::Open | Kind::InspectRecovery) {
                    dialog.pick_file().await
                } else {
                    dialog.save_file().await
                }
            });
            let mut path = file.map(|f| f.path().to_path_buf());
            if let Some(path) = &mut path
                && path.extension().is_none()
                && !matches!(kind, Kind::Open | Kind::InspectRecovery)
            {
                path.set_extension(fallback_extension);
            }
            let _ = sender.send(Completion { kind, path });
            wake();
        });
    }
    pub fn poll(&mut self) -> Option<Completion> {
        let completion = self.receiver.try_recv().ok()?;
        self.pending = false;
        Some(completion)
    }
}

/// The initial format and missing-extension fallback follow the requested export.
/// An explicit returned extension always remains authoritative.
fn export_filters(suggested: Option<&Path>) -> [(&'static str, &'static [&'static str]); 3] {
    let mut filters: [(&str, &[&str]); 3] = [
        ("PNG image", &["png"]),
        ("BMP image", &["bmp"]),
        ("JPEG image", &["jpg", "jpeg"]),
    ];
    if let Some(extension) = suggested
        .and_then(Path::extension)
        .and_then(|extension| extension.to_str())
        && let Some(index) = filters.iter().position(|(_, extensions)| {
            extensions
                .iter()
                .any(|known| extension.eq_ignore_ascii_case(known))
        })
    {
        filters.swap(0, index);
    }
    filters
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn explicit_export_choices_select_the_matching_filter_and_fallback() {
        for (path, expected) in [
            ("Untitled.png", "png"),
            ("photo.JPEG", "jpg"),
            ("Untitled.bmp", "bmp"),
            ("unknown", "png"),
        ] {
            let filters = export_filters(Some(Path::new(path)));
            assert_eq!(filters[0].1[0], expected);
            assert_eq!(
                filters
                    .iter()
                    .map(|(_, extensions)| extensions.len())
                    .sum::<usize>(),
                4
            );
        }
        assert_eq!(export_filters(None)[0].1[0], "png");
    }
}
