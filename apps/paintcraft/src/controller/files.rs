//! File association and synchronous saves; asynchronous coordination lives in saving.
use super::*;

impl Editor {
    pub fn open(path: &Path) -> Result<Editor> {
        let (bytes, binding) = crate::destination::acquire(path)?;
        let mut e = Editor::with_document(pcnd::read(&bytes)?, Some(path.to_path_buf()));
        e.destination = Some(binding);
        e.status = format!("Opened {}", path.display());
        Ok(e)
    }

    pub fn path(&self) -> Option<&Path> {
        self.path.as_deref()
    }

    pub fn set_path(&mut self, path: PathBuf) {
        if self.path.as_ref() != Some(&path) {
            self.destination = Some(crate::destination::Binding::authorized());
        }
        self.path = Some(path);
    }

    pub(crate) fn destination_binding(&self) -> Option<crate::destination::Binding> {
        self.destination.clone()
    }
    pub(crate) fn bind_destination(&mut self, binding: Option<crate::destination::Binding>) {
        self.destination = binding;
    }
    pub(crate) fn file_association(&self) -> crate::destination::Previous {
        crate::destination::Previous {
            path: self.path.clone(),
            binding: self.destination.clone(),
        }
    }
    pub(crate) fn restore_path(&mut self, previous: crate::destination::Previous) {
        self.path = previous.path;
        self.destination = previous.binding;
    }

    pub(super) fn save(&mut self) {
        self.save_outcome = Some(false);
        let Some(path) = self.path.clone() else {
            self.status = "Choose a file name before saving".into();
            return;
        };
        let Some(binding) = self.destination.clone() else {
            self.status = "Not saved safely: choose Save As to authorize a destination".into();
            return;
        };
        let outcome = binding.save(|expected| {
            save::save_with_expectation_and_preparation(
                &path,
                true,
                save::STAGED_FILE_BYTES_MAX,
                expected,
                |sink| pcnd::write_compact_to(self.document(), sink),
                |_| Ok(()),
            )
        });
        if outcome.is_success() {
            self.save_outcome = Some(true);
            self.session.mark_saved();
            self.status = match self.discard_checkpoint() {
                Ok(()) => format!("Saved {}", path.display()),
                Err(error) => format!("Saved {}; recovery cleanup failed: {error}", path.display()),
            };
        } else {
            // Dirty state stays: an unconfirmed or unknown save is not a clean document.
            self.status = format!("Not saved safely: {}", outcome.reasons.join("; "));
        }
    }
}
