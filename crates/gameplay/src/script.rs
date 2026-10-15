use std::collections::HashMap;

/// Yield status returned by a script step indicating how it wants to be resumed.
pub enum ScriptYield {
    /// Resumes on the next frame immediately.
    Yield,
    /// Suspends execution for the specified number of simulation frames.
    WaitFrames(u32),
    /// Suspends execution for the given duration in seconds.
    WaitSeconds(f32),
    /// Suspends execution until the provided condition closure evaluates to true.
    WaitUntil(Box<dyn Fn(&ScriptContext) -> bool + Send + Sync>),
    /// Signals that the script has completed and should be terminated.
    Done,
}

/// Shared execution context passed to script steps (RAGE script globals & engine events).
#[derive(Default)]
pub struct ScriptContext {
    pub delta_time: f32,
    pub total_time: f32,
    pub frame_count: u64,
    /// Script globals / state flags accessible across all running fibers.
    pub globals: HashMap<String, i64>,
    pub string_globals: HashMap<String, String>,
    /// Custom event triggers dispatched by scripts.
    pub triggered_events: Vec<String>,
}

impl ScriptContext {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn set_global(&mut self, key: impl Into<String>, value: i64) {
        self.globals.insert(key.into(), value);
    }

    pub fn get_global(&self, key: &str) -> i64 {
        self.globals.get(key).copied().unwrap_or(0)
    }

    pub fn set_string(&mut self, key: impl Into<String>, value: impl Into<String>) {
        self.string_globals.insert(key.into(), value.into());
    }

    pub fn get_string(&self, key: &str) -> Option<&str> {
        self.string_globals.get(key).map(|s| s.as_str())
    }

    pub fn emit_event(&mut self, event: impl Into<String>) {
        self.triggered_events.push(event.into());
    }
}

enum FiberWaitCondition {
    None,
    Frames(u32),
    Seconds(f32),
    Predicate(Box<dyn Fn(&ScriptContext) -> bool + Send + Sync>),
}

/// A lightweight cooperatively scheduled script fiber (RAGE .ysc thread equivalent).
pub struct ScriptFiber {
    pub id: u32,
    pub name: String,
    pub is_finished: bool,
    wait_condition: FiberWaitCondition,
    step_fn: Box<dyn FnMut(&mut ScriptContext) -> ScriptYield + Send + Sync>,
}

impl ScriptFiber {
    pub fn new<F>(id: u32, name: impl Into<String>, step_fn: F) -> Self
    where
        F: FnMut(&mut ScriptContext) -> ScriptYield + Send + Sync + 'static,
    {
        Self {
            id,
            name: name.into(),
            is_finished: false,
            wait_condition: FiberWaitCondition::None,
            step_fn: Box::new(step_fn),
        }
    }

    /// Evaluates waiting conditions and executes one step if ready.
    pub fn tick(&mut self, ctx: &mut ScriptContext) {
        if self.is_finished {
            return;
        }

        let ready = match &mut self.wait_condition {
            FiberWaitCondition::None => true,
            FiberWaitCondition::Frames(frames) => {
                if *frames <= 1 {
                    true
                } else {
                    *frames -= 1;
                    false
                }
            }
            FiberWaitCondition::Seconds(secs) => {
                *secs -= ctx.delta_time;
                *secs <= 0.0
            }
            FiberWaitCondition::Predicate(pred) => pred(ctx),
        };

        if ready {
            self.wait_condition = FiberWaitCondition::None;
            match (self.step_fn)(ctx) {
                ScriptYield::Yield => {
                    self.wait_condition = FiberWaitCondition::None;
                }
                ScriptYield::WaitFrames(f) => {
                    self.wait_condition = FiberWaitCondition::Frames(f);
                }
                ScriptYield::WaitSeconds(s) => {
                    self.wait_condition = FiberWaitCondition::Seconds(s);
                }
                ScriptYield::WaitUntil(pred) => {
                    self.wait_condition = FiberWaitCondition::Predicate(pred);
                }
                ScriptYield::Done => {
                    self.is_finished = true;
                }
            }
        }
    }
}

/// Cooperative script manager running gameplay mission threads and ambient cutscenes.
pub struct ScriptManager {
    pub context: ScriptContext,
    fibers: Vec<ScriptFiber>,
    next_fiber_id: u32,
}

impl Default for ScriptManager {
    fn default() -> Self {
        Self::new()
    }
}

impl ScriptManager {
    pub fn new() -> Self {
        Self {
            context: ScriptContext::new(),
            fibers: Vec::new(),
            next_fiber_id: 1,
        }
    }

    /// Spawns a new cooperative script fiber into the scheduler.
    pub fn spawn_fiber<F>(&mut self, name: impl Into<String>, step_fn: F) -> u32
    where
        F: FnMut(&mut ScriptContext) -> ScriptYield + Send + Sync + 'static,
    {
        let id = self.next_fiber_id;
        self.next_fiber_id += 1;
        self.fibers.push(ScriptFiber::new(id, name, step_fn));
        id
    }

    /// Ticks all active fibers and advances context time.
    pub fn update(&mut self, dt: f32) {
        self.context.delta_time = dt;
        self.context.total_time += dt;
        self.context.frame_count += 1;
        self.context.triggered_events.clear();

        for fiber in &mut self.fibers {
            fiber.tick(&mut self.context);
        }

        // Clean up completed fibers
        self.fibers.retain(|f| !f.is_finished);
    }

    pub fn active_fiber_count(&self) -> usize {
        self.fibers.len()
    }

    pub fn terminate_fiber(&mut self, id: u32) {
        if let Some(fiber) = self.fibers.iter_mut().find(|f| f.id == id) {
            fiber.is_finished = true;
        }
    }
}
