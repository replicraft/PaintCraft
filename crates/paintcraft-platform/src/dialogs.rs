//! Parented native dialogs. Completion is polled by the application event loop.
use raw_window_handle::{HasDisplayHandle, HasWindowHandle};
use std::{
    path::{Path, PathBuf},
    sync::mpsc,
};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Kind {
    Open,
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
        dialog = match kind {
            Kind::Open => dialog
                .set_title("Open image or PaintCraft document")
                .add_filter(
                    "Images and PaintCraft documents",
                    &["pcnd", "png", "bmp", "jpg", "jpeg"],
                ),
            Kind::SaveAs => dialog
                .set_title("Save PaintCraft document")
                .add_filter("PaintCraft document", &["pcnd"]),
            Kind::Export => dialog
                .set_title("Export image")
                .add_filter("PNG image", &["png"])
                .add_filter("BMP image", &["bmp"])
                .add_filter("JPEG image", &["jpg", "jpeg"]),
        };
        self.pending = true;
        let sender = self.sender.clone();
        std::thread::spawn(move || {
            let file = pollster::block_on(async {
                if kind == Kind::Open {
                    dialog.pick_file().await
                } else {
                    dialog.save_file().await
                }
            });
            let mut path = file.map(|f| f.path().to_path_buf());
            if let Some(path) = &mut path
                && path.extension().is_none()
                && kind != Kind::Open
            {
                path.set_extension(if kind == Kind::SaveAs { "pcnd" } else { "png" });
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
