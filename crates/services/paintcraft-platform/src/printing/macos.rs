use super::*;
use std::{
    fs,
    io::Write,
    os::unix::fs::{DirBuilderExt, PermissionsExt},
    path::{Path, PathBuf},
    process::Command,
};
const HELPER: &[u8] = include_bytes!(concat!(env!("OUT_DIR"), "/paintcraft-print"));
struct Directory(PathBuf);
impl Directory {
    fn new() -> Result<Self> {
        let path = std::env::temp_dir().join(format!("paintcraft-print-{}", uuid::Uuid::new_v4()));
        fs::DirBuilder::new()
            .mode(0o700)
            .create(&path)
            .map_err(|error| format!("Cannot prepare native printing: {error}"))?;
        Ok(Self(path))
    }
    fn file(&self, name: &str, bytes: &[u8], mode: u32) -> Result<PathBuf> {
        let path = self.0.join(name);
        let mut file = fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&path)
            .map_err(|error| error.to_string())?;
        file.set_permissions(fs::Permissions::from_mode(mode))
            .map_err(|error| error.to_string())?;
        file.write_all(bytes).map_err(|error| error.to_string())?;
        Ok(path)
    }
}
impl Drop for Directory {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}
fn command(helper: &Path, png: &Path, title: &str) -> Command {
    let mut command = Command::new(helper);
    command.arg("--png").arg(png).arg("--title").arg(title);
    command
}
pub(super) fn run(png: Vec<u8>, title: String) -> Result<PrintOutcome> {
    validate_png(&png)?;
    let directory = Directory::new()?;
    let helper = directory.file("paintcraft-print", HELPER, 0o700)?;
    let image = directory.file("image.png", &png, 0o600)?;
    let status = command(&helper, &image, &title)
        .status()
        .map_err(|error| format!("Cannot open native print settings: {error}"))?;
    match status.code() {
        Some(0) => Ok(PrintOutcome::Printed),
        Some(2) => Ok(PrintOutcome::Cancelled),
        _ => Err("Native printing failed before completing the requested job".into()),
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn helper_validates_encoded_image_without_opening_dialogs_or_submitting_jobs() {
        let directory = Directory::new().unwrap();
        let helper = directory.file("paintcraft-print", HELPER, 0o700).unwrap();
        let image = directory
            .file("image.png", &crate::printing::tests::PNG, 0o600)
            .unwrap();
        let original = directory.0.clone();
        assert_eq!(
            fs::metadata(&original).unwrap().permissions().mode() & 0o777,
            0o700
        );
        let title = "Title with spaces; $(touch /tmp/never-run) '日本語'";
        let mut process = command(&helper, &image, title);
        let args: Vec<_> = process.get_args().map(|arg| arg.to_owned()).collect();
        assert_eq!(args[3], title);
        let status = process.arg("--validate-only").status().unwrap();
        assert!(status.success());
        fs::write(&image, b"invalid input").unwrap();
        assert!(
            !command(&helper, &image, title)
                .arg("--validate-only")
                .status()
                .unwrap()
                .success()
        );
        drop(directory);
        assert!(!original.exists());
    }
}
