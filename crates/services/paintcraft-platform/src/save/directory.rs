//! Open directory descriptors with the access needed for a persistence barrier.

use std::fs::File;
use std::path::Path;

#[cfg(not(windows))]
pub(super) fn open(path: &Path) -> std::io::Result<File> {
    File::open(path)
}

#[cfg(windows)]
pub(super) fn open(path: &Path) -> std::io::Result<File> {
    use std::fs::OpenOptions;
    use std::os::windows::fs::OpenOptionsExt;

    // A plain File::open cannot open directories on Windows. FlushFileBuffers
    // also requires write access (FILE_ADD_FILE is FILE_WRITE_DATA for a directory).
    const FILE_LIST_DIRECTORY: u32 = 0x0001;
    const FILE_ADD_FILE: u32 = 0x0002;
    const SYNCHRONIZE: u32 = 0x0010_0000;
    const FILE_FLAG_BACKUP_SEMANTICS: u32 = 0x0200_0000;
    OpenOptions::new()
        .access_mode(FILE_LIST_DIRECTORY | FILE_ADD_FILE | SYNCHRONIZE)
        .custom_flags(FILE_FLAG_BACKUP_SEMANTICS)
        .open(path)
}

#[cfg(all(test, windows))]
mod tests {
    #[test]
    fn directory_descriptor_can_flush_a_new_entry_and_rename() {
        let root = super::super::tests::scratch("windows-directory-sync");
        let file = root.join("before.pcnd");
        std::fs::write(&file, b"saved").unwrap();
        let directory = super::open(&root).unwrap();
        assert!(directory.metadata().unwrap().is_dir());
        directory.sync_all().unwrap();
        std::fs::rename(&file, root.join("after.pcnd")).unwrap();
        directory.sync_all().unwrap();
        drop(directory);
        std::fs::remove_dir_all(root).unwrap();
    }
}
