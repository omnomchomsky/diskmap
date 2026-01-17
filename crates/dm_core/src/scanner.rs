use std::ffi::OsString;
use std::io;
use std::path::{Path, PathBuf};
use crate::fs_adapter::FsAdapter;
use crate::model::NodeId;

#[cfg(unix)]
use std::os::unix::fs::MetadataExt;

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
                emit(ScanEvent::Error { node: parent_id, path: path, kind: e.kind() });
                continue;
            }
        };

        if ft.is_file() {
            let size = match entry.metadata() {
                Ok(md) => {
                    #[cfg(unix)]
                    {
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
                    emit(ScanEvent::Error { node: parent_id, path: path, kind: e.kind() });
                    continue;
                }
            };
            emit(ScanEvent::File { parent_id, name, size });
        } else if ft.is_dir() {
            let traversable = match fs.is_traversable_dir(&entry, &ft) {
                Ok(traversable) => traversable,
                Err(e) => {
                    emit(ScanEvent::Error { node: parent_id, path: path, kind: e.kind() });
                    continue;
                }
            };
            if traversable {
                emit(ScanEvent::Dir { parent_id, name, path });
            } else {

            }
        } else {

        }
    }
    emit(ScanEvent::Done { node: parent_id });
}