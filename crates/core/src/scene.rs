use std::collections::HashMap;
use parking_lot::RwLock;
use ow_math::Transform;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct NodeId(pub u64);

#[derive(Debug, Clone)]
pub struct SceneNode {
    pub id: NodeId,
    pub name: String,
    pub transform: Transform,
    pub parent: Option<NodeId>,
    pub children: Vec<NodeId>,
    pub is_active: bool,
    pub layer: u32,
    pub tags: Vec<String>,
}

impl SceneNode {
    pub fn new(id: NodeId, name: impl Into<String>) -> Self {
        Self {
            id,
            name: name.into(),
            transform: Transform::identity(),
            parent: None,
            children: Vec::new(),
            is_active: true,
            layer: 0,
            tags: Vec::new(),
        }
    }

    pub fn with_transform(mut self, transform: Transform) -> Self {
        self.transform = transform;
        self
    }

    pub fn with_parent(mut self, parent: NodeId) -> Self {
        self.parent = Some(parent);
        self
    }

    pub fn with_layer(mut self, layer: u32) -> Self {
        self.layer = layer;
        self
    }

    pub fn with_tags(mut self, tags: Vec<String>) -> Self {
        self.tags = tags;
        self
    }

    pub fn add_child(&mut self, child: NodeId) {
        if !self.children.contains(&child) {
            self.children.push(child);
        }
    }

    pub fn remove_child(&mut self, child: NodeId) {
        self.children.retain(|&c| c != child);
    }

    pub fn has_child(&self, child: NodeId) -> bool {
        self.children.contains(&child)
    }

    pub fn child_count(&self) -> usize {
        self.children.len()
    }

    pub fn is_root(&self) -> bool {
        self.parent.is_none()
    }

    pub fn set_active(&mut self, active: bool) {
        self.is_active = active;
    }

    pub fn is_active(&self) -> bool {
        self.is_active
    }
}

pub struct Scene {
    pub name: String,
    nodes: RwLock<HashMap<NodeId, SceneNode>>,
    root_nodes: RwLock<Vec<NodeId>>,
    next_node_id: RwLock<u64>,
}

impl Scene {
    pub fn new(name: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            nodes: RwLock::new(HashMap::new()),
            root_nodes: RwLock::new(Vec::new()),
            next_node_id: RwLock::new(0),
        }
    }

    pub fn create_node(&self, name: impl Into<String>) -> NodeId {
        let mut next_id = self.next_node_id.write();
        let id = NodeId(*next_id);
        *next_id += 1;

        let node = SceneNode::new(id, name);
        self.nodes.write().insert(id, node);
        self.root_nodes.write().push(id);
        id
    }

    pub fn destroy_node(&self, id: NodeId) {
        let mut nodes = self.nodes.write();
        if let Some(node) = nodes.get(&id) {
            let children = node.children.clone();
            for child in children {
                self.destroy_node(child);
            }
        }
        nodes.remove(&id);
        self.root_nodes.write().retain(|&n| n != id);
    }

    pub fn get_node(&self, id: NodeId) -> Option<SceneNode> {
        self.nodes.read().get(&id).cloned()
    }

    pub fn get_node_mut(&self, id: NodeId) -> Option<SceneNode> {
        self.nodes.write().get_mut(&id).cloned()
    }

    pub fn set_parent(&self, child: NodeId, parent: Option<NodeId>) {
        let mut nodes = self.nodes.write();

        if let Some(node) = nodes.get(&child) {
            if let Some(old_parent) = node.parent {
                if let Some(parent_node) = nodes.get_mut(&old_parent) {
                    parent_node.remove_child(child);
                }
            }
        }

        if let Some(parent_id) = parent {
            if let Some(parent_node) = nodes.get_mut(&parent_id) {
                parent_node.add_child(child);
            }
        }

        if let Some(node) = nodes.get_mut(&child) {
            node.parent = parent;
        }

        match parent {
            Some(_) => {
                self.root_nodes.write().retain(|&n| n != child);
            }
            None => {
                let mut roots = self.root_nodes.write();
                if !roots.contains(&child) {
                    roots.push(child);
                }
            }
        }
    }

    pub fn node_count(&self) -> usize {
        self.nodes.read().len()
    }

    pub fn root_count(&self) -> usize {
        self.root_nodes.read().len()
    }

    pub fn get_root_nodes(&self) -> Vec<NodeId> {
        self.root_nodes.read().clone()
    }

    pub fn get_children(&self, id: NodeId) -> Vec<NodeId> {
        self.nodes
            .read()
            .get(&id)
            .map(|n| n.children.clone())
            .unwrap_or_default()
    }

    pub fn find_by_name(&self, name: &str) -> Option<NodeId> {
        self.nodes
            .read()
            .iter()
            .find(|(_, n)| n.name == name)
            .map(|(id, _)| *id)
    }

    pub fn find_by_tag(&self, tag: &str) -> Vec<NodeId> {
        self.nodes
            .read()
            .iter()
            .filter(|(_, n)| n.tags.contains(&tag.to_string()))
            .map(|(id, _)| *id)
            .collect()
    }
}
