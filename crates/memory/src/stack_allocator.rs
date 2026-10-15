use std::alloc::{alloc, dealloc, Layout};
use std::ptr::NonNull;

pub struct StackAllocator {
    start: *mut u8,
    end: *mut u8,
    current: *mut u8,
}

impl StackAllocator {
    pub fn new(size: usize) -> Self {
        let layout = Layout::from_size_align(size, 8).unwrap();
        let start = unsafe { alloc(layout) };
        if start.is_null() {
            panic!("StackAllocator: failed to allocate memory");
        }
        let end = unsafe { start.add(size) };
        Self {
            start,
            end,
            current: start,
        }
    }

    pub fn alloc(&mut self, layout: Layout) -> Option<NonNull<u8>> {
        let align = layout.align();
        let padding = (align - (self.current as usize % align)) % align;
        let new_current = unsafe { self.current.add(padding + layout.size()) };

        if new_current > self.end {
            return None;
        }

        let ptr = unsafe { self.current.add(padding) };
        self.current = new_current;
        Some(unsafe { NonNull::new_unchecked(ptr) })
    }

    pub fn get_marker(&self) -> usize {
        self.current as usize
    }

    pub fn free_to_marker(&mut self, marker: usize) {
        if marker >= self.start as usize && marker <= self.end as usize {
            self.current = marker as *mut u8;
        }
    }

    pub fn used(&self) -> usize {
        (self.current as usize) - (self.start as usize)
    }

    pub fn capacity(&self) -> usize {
        (self.end as usize) - (self.start as usize)
    }
}

impl Drop for StackAllocator {
    fn drop(&mut self) {
        let layout = Layout::from_size_align(self.capacity(), 8).unwrap();
        unsafe { dealloc(self.start, layout) };
    }
}
