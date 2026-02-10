use std::fs::{DirEntry, ReadDir};
use std::path::{Path, PathBuf};
use std::io;

pub trait FsAdapter: Send + Sync {
    fn read_dir(&self, path: &Path) -> io::Result<ReadDir>;

    fn entry_path(&self, e: &DirEntry) -> PathBuf {
        e.path()
    }

    fn entry_file_type(&self, e: &DirEntry) -> io::Result<std::fs::FileType> {
        e.file_type()
    }

    fn entry_name(&self, e: &DirEntry) -> std::ffi::OsString {
        e.file_name()
    }

    fn entry_metadata(&self, e: &DirEntry) -> io::Result<std::fs::Metadata> {
        e.metadata()
    }

    /// “Safe to descend into” (symlinks/reparse points => false by default)
    fn is_traversable_dir(&self, e: &DirEntry, ft: &std::fs::FileType) -> io::Result<bool>;
}

#[derive(Clone)]
pub struct UnixFsAdapter;

impl FsAdapter for UnixFsAdapter {
    fn read_dir(&self, path: &Path) -> io::Result<ReadDir> {
        std::fs::read_dir(path)
    }

    fn is_traversable_dir(&self, _e: &DirEntry, ft: &std::fs::FileType) -> io::Result<bool> {
        // On Unix, symlink dirs are the main loop risk.
        Ok(ft.is_dir() && !ft.is_symlink())
    }
}

#[derive(Clone)]
pub struct WindowsFsAdapter;

impl FsAdapter for WindowsFsAdapter {
    fn read_dir(&self, path: &Path) -> io::Result<ReadDir> {
        std::fs::read_dir(path)
    }

    fn is_traversable_dir(&self, e: &DirEntry, ft: &std::fs::FileType) -> io::Result<bool> {
        // Don't traverse symlinks
        if ft.is_symlink() {
            return Ok(false);
        }

        // On Windows, check for reparse points (junctions, symlinks, etc.)
        #[cfg(target_os = "windows")]
        {
            use std::os::windows::fs::MetadataExt;
            const FILE_ATTRIBUTE_REPARSE_POINT: u32 = 0x400;

            if let Ok(metadata) = e.metadata() {
                let attrs = metadata.file_attributes();
                if (attrs & FILE_ATTRIBUTE_REPARSE_POINT) != 0 {
                    return Ok(false);
                }
            }
        }

        Ok(ft.is_dir())
    }
}
