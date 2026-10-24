//! Parented native dialogs. Completion is polled by the application event loop.
use raw_window_handle::{HasDisplayHandle, HasWindowHandle};
use std::{
    path::{Path, PathBuf},
    sync::mpsc,
};

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
        if self.pending {
            return;
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
        let export_filters = export_filters(suggested);
        let fallback_extension = if kind == Kind::SaveAs {
            "pcnd"
        } else {
            export_filters[0].1[0]
        };
        dialog = match kind {
            Kind::Open => dialog
                .set_title(label("Open image or PaintCraft document"))
                .add_filter(
                    label("Images and PaintCraft documents"),
                    &["pcnd", "png", "bmp", "jpg", "jpeg"],
                ),
            Kind::InspectRecovery => dialog
                .set_title(label("Locate the destination of an incomplete save"))
                .add_filter(label("PaintCraft document"), &["pcnd"]),
            Kind::SaveAs => dialog
                .set_title(label("Save PaintCraft document"))
                .add_filter(label("PaintCraft document"), &["pcnd"]),
            Kind::Export => {
                let mut dialog = dialog.set_title(label("Export image"));
                for (name, extensions) in export_filters {
                    dialog = dialog.add_filter(label(name), extensions);
                }
                dialog
            }
        };
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
