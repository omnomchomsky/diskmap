// Prevents additional console window on Windows in release
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use serde::{Deserialize, Serialize};
use dm_core::session::Session;
use dm_core::fs_adapter::{UnixFsAdapter, WindowsFsAdapter};
use dm_core::tree::Tree;
use std::path::PathBuf;
use tauri::Emitter;

#[tauri::command]
async fn scan_directory_parallel_stream(
    window: tauri::Window,
    path: String,
    threads: Option<usize>,
) -> Result<ScanResult, String> {
    let threads = threads.unwrap_or(16);
    let start = std::time::Instant::now();
    let fs_stats = get_filesystem_stats(&path);

    // Parse path once
    let path_buf: PathBuf = path.parse().map_err(|e| format!("Invalid path: {}", e))?;

    // Do the heavy scan off the async thread
    let value = window.clone();
    let res = tauri::async_runtime::spawn_blocking(move || {
        // pick adapter per OS
        #[cfg(unix)]
        let fs = dm_core::fs_adapter::UnixFsAdapter;
        #[cfg(windows)]
        let fs = dm_core::fs_adapter::WindowsFsAdapter;

        let mut session = Session::new(path_buf, 10);

        let fs_total = fs_stats.map(|(t, _)| t);
        let fs_free = fs_stats.map(|(_, f)| f);

        // Emit a progress event ~every 200ms (your callback interval)
        session.run_parallel_with_callback(&fs, threads, |s| {
            let root_id = s.tree.root();
            let cur = s.tree.node(root_id).total_bytes();

            let payload = ScanProgress {
                jobs_started: s.jobs_started,
                jobs_done: s.jobs_done,
                errors: s.errors,
                current_size: cur,
                duration_ms: start.elapsed().as_millis() as u64,
                tree: serialize_tree_limited(&s.tree, root_id, 2),
                root_id,
                fs_total,
                fs_free,
            };

            // Ignore errors if the window is gone
            let _ = window.emit("scan_progress", payload);
        });

        let duration_ms = start.elapsed().as_millis() as u64;
        let root_id = session.tree.root();
        let total_size = session.tree.node(root_id).total_bytes();

        Ok::<_, String>(ScanResult {
            tree: serialize_tree(&session.tree, root_id),
            root_id,
            total_size,
            errors: session.errors,
            duration_ms,
            fs_total,
            fs_free,
        })
    })
        .await
        .map_err(|e| format!("Scan task failed: {}", e))??;

    // Emit a final event too (optional)
    let _ = value.emit("scan_done", &res);

    Ok(res)
}

#[derive(Serialize, Deserialize, Clone)]
struct GuiFileMetaData {
    name: String,
    size: u64,
}

#[derive(Serialize, Deserialize, Clone)]
struct TreeNode {
    id: usize,
    parent: Option<usize>,
    name: String,
    size: u64,
    own_size: u64,
    children: Vec<usize>,
    top_files: Vec<GuiFileMetaData>,
    depth: usize,
}

#[derive(Serialize)]
struct ScanResult {
    tree: Vec<TreeNode>,
    root_id: usize,
    total_size: u64,
    errors: u64,
    duration_ms: u64,
    fs_total: Option<u64>,
    fs_free: Option<u64>,
}

#[derive(Serialize, Clone)]
struct ScanProgress {
    jobs_started: u64,
    jobs_done: u64,
    errors: u64,
    current_size: u64,
    duration_ms: u64,
    tree: Vec<TreeNode>,
    root_id: usize,
    fs_total: Option<u64>,
    fs_free: Option<u64>,
}

fn serialize_tree(tree: &Tree, root_id: usize) -> Vec<TreeNode> {
    let mut nodes = Vec::new();
    let mut stack = vec![(root_id, 0)];

    while let Some((node_id, depth)) = stack.pop() {
        let node = tree.node(node_id);

        nodes.push(TreeNode {
            id: node_id,
            parent: node.parent_id(),
            name: node.name().to_string(),
            size: node.total_bytes(),
            own_size: node.own_bytes(),
            children: node.children().to_vec(),
            top_files: node.top_files().to_sorted_vec_desc().into_iter().map(|f| GuiFileMetaData {
                name: f.name,
                size: f.size,
            }).collect(),
            depth,
        });

        for &child_id in node.children() {
            stack.push((child_id, depth + 1));
        }
    }

    nodes
}

fn serialize_tree_limited(tree: &Tree, root_id: usize, max_depth: usize) -> Vec<TreeNode> {
    let mut nodes = Vec::new();
    let mut stack = vec![(root_id, 0)];

    while let Some((node_id, depth)) = stack.pop() {
        let node = tree.node(node_id);
        let children = if depth < max_depth {
            node.children().to_vec()
        } else {
            Vec::new()
        };

        nodes.push(TreeNode {
            id: node_id,
            parent: node.parent_id(),
            name: node.name().to_string(),
            size: node.total_bytes(),
            own_size: node.own_bytes(),
            children,
            top_files: node.top_files().to_sorted_vec_desc().into_iter().map(|f| GuiFileMetaData {
                name: f.name,
                size: f.size,
            }).collect(),
            depth,
        });

        if depth < max_depth {
            for &child_id in node.children() {
                stack.push((child_id, depth + 1));
            }
        }
    }

    nodes
}

fn get_filesystem_stats(path: &str) -> Option<(u64, u64)> {
    use std::path::Path;

    // Use fs4 crate for cross-platform disk space queries
    let path_obj = Path::new(path);

    // Try to get a canonical path, fall back to the original if that fails
    let canonical_path = std::fs::canonicalize(path_obj).unwrap_or_else(|_| path_obj.to_path_buf());

    match fs4::statvfs(&canonical_path) {
        Ok(stats) => {
            let total = stats.total_space();
            let free = stats.available_space();
            Some((total, free))
        }
        Err(_) => None,
    }
}

#[tauri::command]
async fn scan_directory(path: String) -> Result<ScanResult, String> {
    let start = std::time::Instant::now();
    let fs = UnixFsAdapter;

    let path_buf: PathBuf = path.parse().map_err(|e| format!("Invalid path: {}", e))?;
    let mut session = Session::new(path_buf, 10);

    let fs_stats = get_filesystem_stats(&path);

    // Run the scan
    session.run(&fs);

    let duration_ms = start.elapsed().as_millis() as u64;
    let root_id = session.tree.root();
    let total_size = session.tree.node(root_id).total_bytes();

    Ok(ScanResult {
        tree: serialize_tree(&session.tree, root_id),
        root_id,
        total_size,
        errors: session.errors,
        duration_ms,
        fs_total: fs_stats.map(|(total, _)| total),
        fs_free: fs_stats.map(|(_, free)| free),
    })
}

#[tauri::command]
async fn scan_directory_parallel(path: String, threads: Option<usize>) -> Result<ScanResult, String> {
    let start = std::time::Instant::now();

    #[cfg(unix)]
    let fs = UnixFsAdapter;
    #[cfg(windows)]
    let fs = WindowsFsAdapter;

    let path_buf: PathBuf = path.parse().map_err(|e| format!("Invalid path: {}", e))?;
    let mut session = Session::new(path_buf, 10);

    let fs_stats = get_filesystem_stats(&path);

    // Run the parallel scan
    session.run_parallel(&fs, threads.unwrap_or(16));

    let duration_ms = start.elapsed().as_millis() as u64;
    let root_id = session.tree.root();
    let total_size = session.tree.node(root_id).total_bytes();

    Ok(ScanResult {
        tree: serialize_tree(&session.tree, root_id),
        root_id,
        total_size,
        errors: session.errors,
        duration_ms,
        fs_total: fs_stats.map(|(total, _)| total),
        fs_free: fs_stats.map(|(_, free)| free),
    })
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_dialog::init())
        .invoke_handler(tauri::generate_handler![
            scan_directory,
            scan_directory_parallel,
            scan_directory_parallel_stream,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}

fn main() {
    run();
}
