// Prevents additional console window on Windows in release
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use serde::{Deserialize, Serialize};
use dm_core::session::Session;
use dm_core::fs_adapter::UnixFsAdapter;
use dm_core::tree::Tree;
use std::path::PathBuf;

#[derive(Serialize, Deserialize)]
struct TreeNode {
    id: usize,
    parent: Option<usize>,
    name: String,
    size: u64,
    own_size: u64,
    children: Vec<usize>,
}

#[derive(Serialize)]
struct ScanResult {
    tree: Vec<TreeNode>,
    root_id: usize,
    total_size: u64,
    errors: u64,
    duration_ms: u64,
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
    let mut stack = vec![root_id];

    while let Some(node_id) = stack.pop() {
        let node = tree.node(node_id);

        nodes.push(TreeNode {
            id: node_id,
            parent: node.parent_id(),
            name: node.name().to_string(),
            size: node.total_bytes(),
            own_size: node.own_bytes(),
            children: node.children().to_vec(),
        });

        for &child_id in node.children() {
            stack.push(child_id);
        }
    }

    nodes
}

#[tauri::command]
async fn scan_directory(path: String) -> Result<ScanResult, String> {
    let start = std::time::Instant::now();
    let fs = UnixFsAdapter;

    let path_buf: PathBuf = path.parse().map_err(|e| format!("Invalid path: {}", e))?;
    let mut session = Session::new(path_buf, 10);

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
    })
}

#[tauri::command]
async fn scan_directory_parallel(path: String, threads: usize) -> Result<ScanResult, String> {
    let start = std::time::Instant::now();
    let fs = UnixFsAdapter;

    let path_buf: PathBuf = path.parse().map_err(|e| format!("Invalid path: {}", e))?;
    let mut session = Session::new(path_buf, 10);

    // Run the parallel scan
    session.run_parallel(&fs, threads);

    let duration_ms = start.elapsed().as_millis() as u64;
    let root_id = session.tree.root();
    let total_size = session.tree.node(root_id).total_bytes();

    Ok(ScanResult {
        tree: serialize_tree(&session.tree, root_id),
        root_id,
        total_size,
        errors: session.errors,
        duration_ms,
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
