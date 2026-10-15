use std::collections::HashMap;
use parking_lot::RwLock;
use crate::entity::{Entity, EntityId};
use crate::system::{System, SystemStage, SystemPriority};

pub type SystemEntry = (SystemStage, SystemPriority, Box<dyn System>);

pub struct World {
    entities: RwLock<HashMap<EntityId, Entity>>,
    systems: RwLock<Vec<SystemEntry>>,
    next_entity_id: RwLock<u64>,
}

impl World {
    pub fn new() -> Self {
        Self {
            entities: RwLock::new(HashMap::new()),
            systems: RwLock::new(Vec::new()),
            next_entity_id: RwLock::new(0),
        }
    }

    pub fn create_entity(&self) -> EntityId {
        let mut next_id = self.next_entity_id.write();
        let id = EntityId {
            id: *next_id,
            generation: 0,
        };
        *next_id += 1;

        let entity = Entity::new(id);
        self.entities.write().insert(id, entity);
        id
    }

    pub fn destroy_entity(&self, id: EntityId) {
        self.entities.write().remove(&id);
    }

    pub fn get_entity(&self, id: EntityId) -> Option<Entity> {
        self.entities.read().get(&id).cloned()
    }

    pub fn get_component_store(&self, id: EntityId) -> Option<crate::entity::ComponentStore> {
        self.entities.read().get(&id).map(|e| e.component_store())
    }

    pub fn add_system<S: System + 'static>(&self, system: S) {
        self.systems.write().push((
            system.stage(),
            system.priority(),
            Box::new(system),
        ));
    }

    pub fn run_systems(&mut self, stage: SystemStage, dt: f32) {
        let mut systems = std::mem::take(&mut *self.systems.write());
        systems.sort_by_key(|(_, p, _)| *p);

        for (system_stage, _, system) in systems.iter_mut() {
            if *system_stage == stage {
                system.run(self, dt);
            }
        }

        self.systems.write().extend(systems);
    }

    pub fn entity_count(&self) -> usize {
        self.entities.read().len()
    }

    pub fn system_count(&self) -> usize {
        self.systems.read().len()
    }
}

impl Default for World {
    fn default() -> Self {
        Self::new()
    }
}

pub struct WorldBuilder {
    world: World,
}

impl WorldBuilder {
    pub fn new() -> Self {
        Self {
            world: World::new(),
        }
    }

    pub fn with_system<S: System + 'static>(self, system: S) -> Self {
        self.world.add_system(system);
        self
    }

    pub fn build(self) -> World {
        self.world
    }
}

impl Default for WorldBuilder {
    fn default() -> Self {
        Self::new()
    }
}
