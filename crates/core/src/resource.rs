use std::collections::HashMap;
use std::sync::Arc;
use parking_lot::RwLock;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct ResourceHandle {
    pub id: u64,
    pub resource_type: &'static str,
}

pub trait Resource: Send + Sync + 'static {
    fn resource_type(&self) -> &'static str;
    fn size_in_bytes(&self) -> usize;
    fn is_loaded(&self) -> bool;
    fn unload(&mut self);
}

pub struct ResourceManager {
    resources: RwLock<HashMap<u64, Box<dyn Resource>>>,
    next_id: RwLock<u64>,
    total_memory: RwLock<usize>,
}

impl ResourceManager {
    pub fn new() -> Self {
        Self {
            resources: RwLock::new(HashMap::new()),
            next_id: RwLock::new(0),
            total_memory: RwLock::new(0),
        }
    }

    pub fn register<R: Resource + 'static>(&self, resource: R) -> ResourceHandle {
        let mut next_id = self.next_id.write();
        let id = *next_id;
        *next_id += 1;

        let resource_type = resource.resource_type();
        let size = resource.size_in_bytes();

        self.resources.write().insert(id, Box::new(resource));
        *self.total_memory.write() += size;

        ResourceHandle { id, resource_type }
    }

    pub fn unregister(&self, handle: ResourceHandle) -> bool {
        let mut resources = self.resources.write();
        if let Some(resource) = resources.remove(&handle.id) {
            *self.total_memory.write() -= resource.size_in_bytes();
            true
        } else {
            false
        }
    }

    pub fn get(&self, _handle: ResourceHandle) -> Option<Arc<RwLock<Box<dyn Resource>>>> {
        // Note: In a real implementation, use a proper handle-based storage
        None
    }

    pub fn get_typed<T: Resource + 'static>(&self, _handle: ResourceHandle) -> Option<Arc<RwLock<T>>> {
        // Note: In a real implementation, use a proper typed handle storage
        None
    }

    pub fn resource_count(&self) -> usize {
        self.resources.read().len()
    }

    pub fn total_memory(&self) -> usize {
        *self.total_memory.read()
    }

    pub fn get_resources_by_type(&self, resource_type: &'static str) -> Vec<ResourceHandle> {
        self.resources
            .read()
            .iter()
            .filter(|(_, r)| r.resource_type() == resource_type)
            .map(|(id, _)| ResourceHandle {
                id: *id,
                resource_type,
            })
            .collect()
    }

    pub fn unload_all(&self) {
        let mut resources = self.resources.write();
        for resource in resources.values_mut() {
            resource.unload();
        }
        resources.clear();
        *self.total_memory.write() = 0;
    }
}

impl Default for ResourceManager {
    fn default() -> Self {
        Self::new()
    }
}
