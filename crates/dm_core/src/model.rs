use crate::top_files::TopKFiles;
pub type NodeId = usize;
pub type ResourceName = String;

#[derive(Debug, Clone)]
pub enum NodeState {
    Unseen,
    Partial,
    Complete
}

#[derive(Debug, Clone)]
pub struct Node {
    id: NodeId,
    parent: Option<NodeId>,
    name: ResourceName,
    pub(crate) children: Vec<NodeId>,
    own_files_bytes: u64,
    own_files_count: u64,
    subtree_bytes_known: u64,
    subtree_bytes_unknown: u64,
    state: NodeState,
    top_files: TopKFiles
}

impl Node {
    pub fn new(id: NodeId, parent: Option<NodeId>, name: ResourceName, top_k:usize) -> Self {
        Self { id,
            parent,
            name,
            children: vec![],
            own_files_bytes: 0,
            own_files_count: 0,
            subtree_bytes_known: 0,
            subtree_bytes_unknown: 0,
            state: NodeState::Unseen,
            top_files: TopKFiles::new(top_k) }
    }
    
    pub fn mark_partial(&mut self) {
        if matches!(self.state, NodeState::Unseen) {
            self.state = NodeState::Partial;
        }
    }

    pub fn mark_complete(&mut self) {
        self.state = NodeState::Complete;
        self.subtree_bytes_unknown = 0;
    }

    pub fn add_file(&mut self, name: impl Into<ResourceName>, size: u64) {
        self.own_files_bytes = self.own_files_bytes.saturating_add(size);
        self.own_files_count = self.own_files_count.saturating_add(1);
        self.top_files.offer(name, size);
        self.mark_partial();
    }

    pub fn add_subtree_known(&mut self, delta: u64) {
        self.subtree_bytes_known = self.subtree_bytes_known.saturating_add(delta);
        self.mark_partial();
    }

    pub fn parent_id(&self) -> Option<NodeId> {
        self.parent
    }

    pub fn id(&self) -> NodeId {
        self.id
    }

    pub fn name(&self) -> &str {
        &self.name
    }

    pub fn children(&self) -> &[NodeId] {
        &self.children
    }

    pub fn total_bytes(&self) -> u64 {
        self.own_files_bytes + self.subtree_bytes_known
    }

    pub fn own_bytes(&self) -> u64 {
        self.own_files_bytes
    }

    pub fn subtree_bytes(&self) -> u64 {
        self.subtree_bytes_known
    }

    pub fn top_files(&self) -> &TopKFiles {
        &self.top_files
    }
}