use std::marker::PhantomData;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct PoolHandle {
    pub index: u32,
    pub generation: u32,
}

pub struct Pool<T> {
    items: Vec<Option<T>>,
    generations: Vec<u32>,
    free_list: Vec<u32>,
    _phantom: PhantomData<T>,
}

impl<T> Pool<T> {
    pub fn new() -> Self {
        Self {
            items: Vec::new(),
            generations: Vec::new(),
            free_list: Vec::new(),
            _phantom: PhantomData,
        }
    }

    pub fn with_capacity(capacity: usize) -> Self {
        Self {
            items: Vec::with_capacity(capacity),
            generations: Vec::with_capacity(capacity),
            free_list: Vec::new(),
            _phantom: PhantomData,
        }
    }

    pub fn insert(&mut self, item: T) -> PoolHandle {
        if let Some(index) = self.free_list.pop() {
            let index = index as usize;
            self.items[index] = Some(item);
            PoolHandle {
                index: index as u32,
                generation: self.generations[index],
            }
        } else {
            let index = self.items.len();
            self.items.push(Some(item));
            self.generations.push(0);
            PoolHandle {
                index: index as u32,
                generation: 0,
            }
        }
    }

    pub fn get(&self, handle: PoolHandle) -> Option<&T> {
        if handle.index >= self.items.len() as u32 {
            return None;
        }
        let index = handle.index as usize;
        if self.generations[index] != handle.generation {
            return None;
        }
        self.items[index].as_ref()
    }

    pub fn get_mut(&mut self, handle: PoolHandle) -> Option<&mut T> {
        if handle.index >= self.items.len() as u32 {
            return None;
        }
        let index = handle.index as usize;
        if self.generations[index] != handle.generation {
            return None;
        }
        self.items[index].as_mut()
    }

    pub fn remove(&mut self, handle: PoolHandle) -> Option<T> {
        if handle.index >= self.items.len() as u32 {
            return None;
        }
        let index = handle.index as usize;
        if self.generations[index] != handle.generation {
            return None;
        }
        let item = self.items[index].take();
        if item.is_some() {
            self.generations[index] += 1;
            self.free_list.push(handle.index);
        }
        item
    }

    pub fn len(&self) -> usize {
        self.items.iter().filter(|i| i.is_some()).count()
    }

    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    pub fn capacity(&self) -> usize {
        self.items.capacity()
    }

    pub fn iter(&self) -> impl Iterator<Item = &T> {
        self.items.iter().filter_map(|i| i.as_ref())
    }

    pub fn iter_mut(&mut self) -> impl Iterator<Item = &mut T> {
        self.items.iter_mut().filter_map(|i| i.as_mut())
    }

    pub fn clear(&mut self) {
        self.items.clear();
        self.generations.clear();
        self.free_list.clear();
    }
}

impl<T> Default for Pool<T> {
    fn default() -> Self {
        Self::new()
    }
}
