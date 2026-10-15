use std::any::Any;
use parking_lot::RwLock;

pub trait Event: Send + Sync + 'static {
    fn event_type(&self) -> &'static str;
    fn as_any(&self) -> &dyn Any;
}

pub type ListenerFn = Box<dyn Fn(&dyn Event) + Send + Sync>;

pub struct EventBus {
    listeners: RwLock<Vec<ListenerFn>>,
    event_queue: RwLock<Vec<Box<dyn Event>>>,
}

impl EventBus {
    pub fn new() -> Self {
        Self {
            listeners: RwLock::new(Vec::new()),
            event_queue: RwLock::new(Vec::new()),
        }
    }

    pub fn subscribe<F>(&self, listener: F)
    where
        F: Fn(&dyn Event) + Send + Sync + 'static,
    {
        self.listeners.write().push(Box::new(listener));
    }

    pub fn emit<E: Event>(&self, event: E) {
        self.event_queue.write().push(Box::new(event));
    }

    pub fn process_events(&self) {
        let events: Vec<Box<dyn Event>> = std::mem::take(&mut *self.event_queue.write());
        let listeners = self.listeners.read();

        for event in events {
            for listener in listeners.iter() {
                listener(event.as_ref());
            }
        }
    }

    pub fn listener_count(&self) -> usize {
        self.listeners.read().len()
    }

    pub fn pending_events(&self) -> usize {
        self.event_queue.read().len()
    }
}

impl Default for EventBus {
    fn default() -> Self {
        Self::new()
    }
}

pub trait EventListener {
    fn on_event(&self, event: &dyn Event);
}

#[macro_export]
macro_rules! define_event {
    ($name:ident { $($field:ident : $type:ty),* $(,)? }) => {
        #[derive(Debug, Clone)]
        pub struct $name {
            $(pub $field: $type),*
        }

        impl $crate::event::Event for $name {
            fn event_type(&self) -> &'static str {
                stringify!($name)
            }

            fn as_any(&self) -> &dyn std::any::Any {
                self
            }
        }
    };
}
