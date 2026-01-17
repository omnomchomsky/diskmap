use std::ffi::OsString;
use std::io;
use std::path::{PathBuf};
use std::collections::HashSet;
use crate::fs_adapter::FsAdapter;
use crate::model::NodeId;

#[cfg(unix)]
use std::os::unix::fs::MetadataExt;

#[cfg(target_os = "windows")]
use std::os::windows::fs::MetadataExt;

// File identifier for tracking hard links
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct FileId {
    #[cfg(unix)]
    pub dev: u64,
    #[cfg(unix)]
    pub ino: u64,

    #[cfg(target_os = "windows")]
    pub volume: u64,
    #[cfg(target_os = "windows")]
    pub index: u64,
}

impl FileId {
    #[cfg(unix)]
    pub fn from_metadata(md: &std::fs::Metadata) -> Option<Self> {
        Some(FileId {
            dev: md.dev(),
            ino: md.ino(),
        })
    }

    #[cfg(target_os = "windows")]
    pub fn from_metadata(md: &std::fs::Metadata) -> Option<Self> {
        // Windows file index is only available with special APIs
        // For now, we'll skip hard link tracking on Windows
        // A proper implementation would use GetFileInformationByHandle
        None
    }

    #[cfg(not(any(unix, target_os = "windows")))]
    pub fn from_metadata(_md: &std::fs::Metadata) -> Option<Self> {
        None
    }
}

#[derive(Debug, Clone)]
pub struct ScanJob {
    pub path: PathBuf,
    pub node_id: NodeId,
    pub depth: u16,
}

#[derive(Debug, Clone)]
pub enum ScanEvent {
    File {
        parent_id: NodeId,
        name: OsString,
        size: u64
    },
    Dir {
        parent_id: NodeId,
        name: OsString,
        path: PathBuf
    },
    Done {
        node: NodeId
    },
    Error {
        node: NodeId,
        path: PathBuf,
        kind: io::ErrorKind
    },
}

pub fn scan_one_dir(fs: &dyn FsAdapter, job: ScanJob, emit: &mut dyn FnMut(ScanEvent)){
    let parent_id = job.node_id;

    let rd = match fs.read_dir(&job.path) {
        Ok(rd) => rd,
        Err(e) => {
            emit(ScanEvent::Error { node: parent_id, path: job.path, kind: e.kind() });
            emit(ScanEvent::Done { node: parent_id });
            return;
        }
    };

    for entry_res in rd {
        let entry = match entry_res {
            Ok(entry) => entry,
            Err(e) => {
                emit(ScanEvent::Error { node: parent_id, path: job.path.clone(), kind: e.kind() });
                continue;
            }
        };

        let name = entry.file_name();
        let path = entry.path();

        let ft = match entry.file_type() {
            Ok(ft) => ft,
            Err(e) => {
                emit(ScanEvent::Error { node: parent_id, path, kind: e.kind() });
                continue;
            }
        };

        // Skip symlinks for files too
        if ft.is_symlink() {
            continue;
        }

        if ft.is_file() {
            let size = match entry.metadata() {
                Ok(md) => {
                    // Check for reparse points on Windows files too
                    #[cfg(target_os = "windows")]
                    {
                        const FILE_ATTRIBUTE_REPARSE_POINT: u32 = 0x400;
                        if (md.file_attributes() & FILE_ATTRIBUTE_REPARSE_POINT) != 0 {
                            continue;
                        }
                    }

                    // Skip hard links with nlink > 1 to avoid double counting
                    // Note: This means we won't count ANY instance of hard-linked files
                    // A better approach would track seen inodes, but that requires shared state
                    #[cfg(unix)]
                    {
                        if md.nlink() > 1 {
                            // Skip files with multiple hard links to avoid double counting
                            continue;
                        }
                        // On Unix, st_blocks is in 512-byte units
                        md.blocks() * 512
                    }
                    #[cfg(not(unix))]
                    {
                        // Fallback: approximate with len() rounded up to 4KB blocks
                        let len = md.len();
                        if len == 0 {
                            0
                        } else {
                            ((len + 4095) / 4096) * 4096
                        }
                    }
                },
                Err(e) => {
                    emit(ScanEvent::Error { node: parent_id, path, kind: e.kind() });
                    continue;
                }
            };
            emit(ScanEvent::File { parent_id, name, size });
        } else if ft.is_dir() {
            let traversable = match fs.is_traversable_dir(&entry, &ft) {
                Ok(traversable) => traversable,
                Err(e) => {
                    emit(ScanEvent::Error { node: parent_id, path, kind: e.kind() });
                    continue;
                }
            };
            if traversable {
                emit(ScanEvent::Dir { parent_id, name, path });
            }
        }
    }
    emit(ScanEvent::Done { node: parent_id });
}