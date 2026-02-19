use std::fmt::Debug;
use crate::model::{NodeId, Node, ResourceName};

#[derive(Debug)]
pub struct Tree {
    root: NodeId,
    nodes: Vec<Node>
}

impl Tree {
    pub fn new(root_name:ResourceName, top_k:usize) -> Self {
        let mut node = Vec::new();
        let root_id = 0;
        node.push(Node::new(root_id, None, root_name, top_k));
        Self { root: root_id, nodes: node }
    }

    pub fn add_child(&mut self, parent_id: NodeId, name: ResourceName, top_k:usize) -> NodeId {
        let id = self.nodes.len();
        self.nodes.push(Node::new(id, Some(parent_id), name, top_k));
        self.nodes[parent_id].children.push(id);
        id
    }

    pub fn node(&self, id: NodeId) -> &Node {
        &self.nodes[id]
    }

    pub fn node_mut(&mut self, id: NodeId) -> &mut Node {
        &mut self.nodes[id]
    }

    pub fn root(&self) -> NodeId {
        self.root
    }
}