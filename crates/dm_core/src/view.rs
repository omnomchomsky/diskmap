use crate::tree::Tree;
use crate::model::NodeId;
use serde::{Deserialize, Serialize};

#[derive(Clone, Copy, Debug)]
pub struct BudgetTier {
    pub max_nodes: usize,
    pub max_children: usize,
    pub top_files_k: usize,
}

impl BudgetTier {
    pub const RESCUE: BudgetTier = BudgetTier {
        max_nodes: 10_000,
        max_children: 400,
        top_files_k: 25,
    };

    pub const NORMAL: BudgetTier = BudgetTier {
        max_nodes: 50_000,
        max_children: 1000,
        top_files_k: 50,
    };

    pub const BEEFY: BudgetTier = BudgetTier {
        max_nodes: 200_000,
        max_children: 5000,
        top_files_k: 100,
    };
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct ViewNode {
    pub id: usize,
    pub parent: Option<usize>,
    pub name: String,
    pub size: u64,
    pub own_size: u64,
    pub children: Vec<usize>,
    pub top_files: Vec<FileMetadata>,
    pub depth: usize,
    pub is_other: bool,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct FileMetadata {
    pub name: String,
    pub size: u64,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct ViewPayload {
    pub nodes: Vec<ViewNode>,
    pub root_id: usize,
}

pub fn generate_view(
    tree: &Tree,
    root_id: NodeId,
    depth_limit: usize,
    budget: BudgetTier,
) -> ViewPayload {
    let mut nodes = Vec::new();

    // First, add the parent of the root if it exists (for navigation back)
    let root_node = tree.node(root_id);
    let actual_parent_id = root_node.parent_id();
    if let Some(parent_id) = actual_parent_id {
        let parent_node = tree.node(parent_id);
        nodes.push(ViewNode {
            id: parent_id,
            parent: parent_node.parent_id(),
            name: parent_node.name().to_string(),
            size: parent_node.total_bytes(),
            own_size: parent_node.own_bytes(),
            children: vec![root_id], // Only include the current root as child
            top_files: Vec::new(),
            depth: 0,
            is_other: false,
        });
    }

    let mut stack = vec![(root_id, 0, actual_parent_id)];

    while let Some((node_id, depth, parent_id)) = stack.pop() {
        let node = tree.node(node_id);

        // Get all children sorted by size (no budget limits)
        let mut children: Vec<(usize, u64)> = node
            .children()
            .iter()
            .map(|&id| (id, tree.node(id).total_bytes()))
            .collect();
        children.sort_by(|a, b| b.1.cmp(&a.1));

        if depth == 0 {
            eprintln!("View generation: Root node {} ('{}') has {} children",
                node_id, node.name(), children.len());
            for (i, (child_id, size)) in children.iter().take(10).enumerate() {
                eprintln!("  Child {}: id={}, name='{}', size={}",
                    i, child_id, tree.node(*child_id).name(), size);
            }
        }

        let children_ids: Vec<usize> = children.iter().map(|(id, _)| *id).collect();

        // Get all top files (no budget limits)
        let top_files: Vec<FileMetadata> = node
            .top_files()
            .to_sorted_vec_desc()
            .into_iter()
            .map(|f| FileMetadata {
                name: f.name,
                size: f.size,
            })
            .collect();

        nodes.push(ViewNode {
            id: node_id,
            parent: parent_id,
            name: node.name().to_string(),
            size: node.total_bytes(),
            own_size: node.own_bytes(),
            children: children_ids.clone(),
            top_files,
            depth,
            is_other: false,
        });

        // Push all children to stack if within depth limit
        if depth < depth_limit {
            for (child_id, _) in children.iter().rev() {
                stack.push((*child_id, depth + 1, Some(node_id)));
            }
        }
    }

    ViewPayload {
        nodes,
        root_id,
    }
}

// Lightweight progress node for streaming during scan
#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct ProgressNode {
    pub id: usize,
    pub parent: Option<usize>,
    pub name: String,
    pub size: u64,
    pub own_size: u64,
    pub child_count: u32,
    pub top_child_ids: Vec<usize>,
    pub depth: usize,
}

pub fn generate_progress_view(
    tree: &Tree,
    root_id: NodeId,
    max_depth: usize,
    max_children_per_node: usize,
) -> Vec<ProgressNode> {
    let mut nodes = Vec::new();
    let mut stack = vec![(root_id, 0, None)];

    while let Some((node_id, depth, parent_id)) = stack.pop() {
        let node = tree.node(node_id);

        // Get top children sorted by size
        let mut children: Vec<(usize, u64)> = node
            .children()
            .iter()
            .map(|&id| (id, tree.node(id).total_bytes()))
            .collect();
        children.sort_by(|a, b| b.1.cmp(&a.1));

        let child_count = children.len() as u32;
        let top_child_ids: Vec<usize> = children
            .iter()
            .take(max_children_per_node)
            .map(|(id, _)| *id)
            .collect();

        nodes.push(ProgressNode {
            id: node_id,
            parent: parent_id,
            name: node.name().to_string(),
            size: node.total_bytes(),
            own_size: node.own_bytes(),
            child_count,
            top_child_ids: top_child_ids.clone(),
            depth,
        });

        // Push children to stack if within depth limit
        if depth < max_depth {
            for (child_id, _) in top_child_ids.iter().rev().map(|id| (*id, tree.node(*id).total_bytes())) {
                stack.push((child_id, depth + 1, Some(node_id)));
            }
        }
    }

    nodes
}
