// Prevents additional console window on Windows in release
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use serde::{Deserialize, Serialize};
use dm_core::session::Session;
use dm_core::fs_adapter::UnixFsAdapter;
use dm_core::tree::Tree;
use std::path::PathBuf;

#[derive(Serialize, Deserialize)]
struct GuiFileMetaData {
    name: String,
    size: u64,
}

#[derive(Serialize, Deserialize)]
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

#[derive(Serialize)]
struct ScanProgress {
    jobs_started: u64,
    jobs_done: u64,
    errors: u64,
    current_size: u64,
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
    let fs = UnixFsAdapter;

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
            scan_directory_parallel
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}

fn main() {
    run();
}
