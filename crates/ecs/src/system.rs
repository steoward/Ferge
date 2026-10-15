
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum SystemStage {
    PreUpdate,
    Update,
    PostUpdate,
    PreRender,
    Render,
    PostRender,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub struct SystemPriority(pub i32);

impl SystemPriority {
    pub const HIGHEST: Self = Self(i32::MIN);
    pub const HIGH: Self = Self(-1000);
    pub const NORMAL: Self = Self(0);
    pub const LOW: Self = Self(1000);
    pub const LOWEST: Self = Self(i32::MAX);
}

pub trait System: Send + Sync {
    fn stage(&self) -> SystemStage {
        SystemStage::Update
    }

    fn priority(&self) -> SystemPriority {
        SystemPriority::NORMAL
    }

    fn run(&self, world: &mut crate::World, dt: f32);
}

pub trait SystemGroup: Send + Sync {
    fn add_system<S: System + 'static>(&mut self, system: S);
    fn run_systems(&self, stage: SystemStage, world: &mut crate::World, dt: f32);
}
