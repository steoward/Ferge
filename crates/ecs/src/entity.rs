use std::collections::HashMap;
use std::any::{Any, TypeId};
use std::sync::Arc;
use parking_lot::RwLock;

pub type ComponentStore = Arc<RwLock<HashMap<TypeId, Box<dyn Any + Send + Sync>>>>;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct EntityId {
    pub id: u64,
    pub generation: u32,
}

impl EntityId {
    pub fn new(id: u64) -> Self {
        Self { id, generation: 0 }
    }
}

#[derive(Clone)]
pub struct Entity {
    pub id: EntityId,
    components: ComponentStore,
}

impl Entity {
    pub fn new(id: EntityId) -> Self {
        Self {
            id,
            components: Arc::new(RwLock::new(HashMap::new())),
        }
    }

    pub fn from_components(id: EntityId, components: ComponentStore) -> Self {
        Self { id, components }
    }

    pub fn component_store(&self) -> ComponentStore {
        Arc::clone(&self.components)
    }

    pub fn add_component<T: Any + Send + Sync>(&self, component: T) {
        self.components
            .write()
            .insert(TypeId::of::<T>(), Box::new(component));
    }

    pub fn get_component<T: Any + Send + Sync + Clone>(&self) -> Option<T> {
        let guard = self.components.read();
        let boxed = guard.get(&TypeId::of::<T>())?;
        boxed.downcast_ref::<T>().cloned()
    }

    pub fn with_component<T: Any + Send + Sync, R>(
        &self,
        f: impl FnOnce(&T) -> R,
    ) -> Option<R> {
        let guard = self.components.read();
        let boxed = guard.get(&TypeId::of::<T>())?;
        boxed.downcast_ref::<T>().map(f)
    }

    pub fn with_component_mut<T: Any + Send + Sync, R>(
        &self,
        f: impl FnOnce(&mut T) -> R,
    ) -> Option<R> {
        let mut guard = self.components.write();
        let boxed = guard.get_mut(&TypeId::of::<T>())?;
        boxed.downcast_mut::<T>().map(f)
    }

    pub fn has_component<T: Any + Send + Sync>(&self) -> bool {
        self.components.read().contains_key(&TypeId::of::<T>())
    }

    pub fn remove_component<T: Any + Send + Sync>(&self) -> bool {
        self.components.write().remove(&TypeId::of::<T>()).is_some()
    }

    pub fn component_count(&self) -> usize {
        self.components.read().len()
    }
}

pub struct EntityBuilder {
    entity_id: Option<EntityId>,
    components: Vec<Box<dyn Any + Send + Sync>>,
}

impl EntityBuilder {
    pub fn new() -> Self {
        Self {
            entity_id: None,
            components: Vec::new(),
        }
    }

    pub fn with_id(mut self, id: EntityId) -> Self {
        self.entity_id = Some(id);
        self
    }

    pub fn with_component<T: Any + Send + Sync>(mut self, component: T) -> Self {
        self.components.push(Box::new(component));
        self
    }

    pub fn build(self) -> (EntityId, Vec<Box<dyn Any + Send + Sync>>) {
        let id = self.entity_id.unwrap_or_else(|| EntityId::new(0));
        (id, self.components)
    }
}

impl Default for EntityBuilder {
    fn default() -> Self {
        Self::new()
    }
}
