use std::collections::VecDeque;
use std::path::{Path, PathBuf};

use crate::fs_adapter::FsAdapter;
use crate::scanner::{scan_one_dir, ScanEvent, ScanJob};
use crate::tree::Tree;
use crate::model::{NodeId, ResourceName};

#[derive(Debug)]
pub struct Session {
    pub tree: Tree,
    queue: VecDeque<ScanJob>,
    top_k: usize,

    pub errors: u64,
    pub jobs_started: u64,
    pub jobs_done: u64,
}

impl Session {
    pub fn new(root_path: PathBuf, top_k: usize) -> Self {
        // For now: root node name is the path string (lossy is fine for display)
        let root_name: ResourceName = root_path.to_string_lossy().to_string();

        let tree = Tree::new(root_name, top_k);
        let mut queue = VecDeque::new();
        queue.push_back(ScanJob {
            path: root_path,
            node_id: 0,
            depth: 0,
        });

        Self {
            tree,
            queue,
            top_k,
            errors: 0,
            jobs_started: 0,
            jobs_done: 0,
        }
    }

    pub fn run(&mut self, fs: &dyn FsAdapter) {
        while let Some(job) = self.queue.pop_front() {
            self.jobs_started += 1;

            scan_one_dir(fs, job, &mut |event| {
                self.apply_event(event);
            });
        }
    }

    fn apply_event(&mut self, event: ScanEvent) {
        match event {
            ScanEvent::File { parent_id, name, size } => {
                // Update node
                let node = self.tree.node_mut(parent_id);
                node.add_file(name.to_string_lossy().to_string(), size);

                // Propagate known bytes up the tree (including this node)
                self.propagate_known_delta(parent_id, size);
            }

            ScanEvent::Dir { parent_id, name, path } => {
                // Create child node in tree
                let child_name = name.to_string_lossy().to_string();
                let child_id = self.tree.add_child(parent_id, child_name, self.top_k);

                // Enqueue scan job
                self.queue.push_back(ScanJob {
                    path,
                    node_id: child_id,
                    depth: 0, // or parent depth + 1 if you store it
                });

                // Mark parent partial (we learned something new)
                self.tree.node_mut(parent_id).mark_partial();
            }

            ScanEvent::Done { node } => {
                self.jobs_done += 1;
                self.tree.node_mut(node).mark_complete();
            }

            ScanEvent::Error { node, .. } => {
                self.errors += 1;
                self.tree.node_mut(node).mark_partial();
            }
        }
    }

    fn propagate_known_delta(&mut self, from: NodeId, delta: u64) {
        // add to this node and all ancestors
        let mut cur = Some(from);
        while let Some(id) = cur {
            let parent = self.tree.node(id).parent_id(); // copy parent before mut borrow
            self.tree.node_mut(id).add_subtree_known(delta);
            cur = parent;
        }
    }
}
