use std::alloc::{alloc, dealloc, Layout};
use std::ptr::NonNull;

pub struct BumpAllocator {
    start: *mut u8,
    end: *mut u8,
    current: *mut u8,
}

impl BumpAllocator {
    pub fn new(size: usize) -> Self {
        let layout = Layout::from_size_align(size, 8).unwrap();
        let start = unsafe { alloc(layout) };
        if start.is_null() {
            panic!("BumpAllocator: failed to allocate memory");
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

        self.current = new_current;
        Some(unsafe { NonNull::new_unchecked(self.current.sub(layout.size())) })
    }

    pub fn reset(&mut self) {
        self.current = self.start;
    }

    pub fn used(&self) -> usize {
        (self.current as usize) - (self.start as usize)
    }

    pub fn capacity(&self) -> usize {
        (self.end as usize) - (self.start as usize)
    }
}

impl Drop for BumpAllocator {
    fn drop(&mut self) {
        let layout = Layout::from_size_align(self.capacity(), 8).unwrap();
        unsafe { dealloc(self.start, layout) };
    }
}

pub struct FreeBlock {
    pub start: *mut u8,
    pub size: usize,
    pub next: Option<Box<FreeBlock>>,
}

pub struct FreeListAllocator {
    head: Option<Box<FreeBlock>>,
    start: *mut u8,
    end: *mut u8,
}

impl FreeListAllocator {
    pub fn new(size: usize) -> Self {
        let layout = Layout::from_size_align(size, 8).unwrap();
        let start = unsafe { alloc(layout) };
        if start.is_null() {
            panic!("FreeListAllocator: failed to allocate memory");
        }
        let end = unsafe { start.add(size) };
        let head = Box::new(FreeBlock {
            start,
            size,
            next: None,
        });
        Self {
            head: Some(head),
            start,
            end,
        }
    }

    pub fn alloc(&mut self, layout: Layout) -> Option<NonNull<u8>> {
        let align = layout.align();
        let size = layout.size();

        let mut current = &mut self.head;
        while current.is_some() {
            let block_start = current.as_ref().unwrap().start;
            let block_size = current.as_ref().unwrap().size;
            let padding = (align - (block_start as usize % align)) % align;
            let total_size = padding + size;

            if block_size >= total_size {
                let result = unsafe { NonNull::new_unchecked(block_start.add(padding)) };

                if block_size == total_size {
                    let next = current.as_mut().unwrap().next.take();
                    *current = next;
                } else {
                    current.as_mut().unwrap().start = unsafe { block_start.add(total_size) };
                    current.as_mut().unwrap().size -= total_size;
                }

                return Some(result);
            }

            current = &mut current.as_mut().unwrap().next;
        }

        None
    }

    pub fn dealloc(&mut self, ptr: NonNull<u8>, layout: Layout) {
        let size = layout.size();
        let ptr = ptr.as_ptr();

        let new_block = Box::new(FreeBlock {
            start: ptr,
            size,
            next: None,
        });

        if self.head.is_none() || ptr < self.head.as_ref().unwrap().start {
            let mut new_block = new_block;
            new_block.next = self.head.take();
            self.head = Some(new_block);
            return;
        }

        let mut current = &mut self.head;
        while current.is_some() && current.as_ref().unwrap().start < ptr {
            current = &mut current.as_mut().unwrap().next;
        }

        let mut new_block = new_block;
        new_block.next = current.take();
        *current = Some(new_block);
    }

    pub fn used(&self) -> usize {
        let mut used = 0;
        let mut current = &self.head;
        while let Some(ref block) = current {
            used += block.size;
            current = &block.next;
        }
        self.capacity() - used
    }

    pub fn capacity(&self) -> usize {
        (self.end as usize) - (self.start as usize)
    }
}

impl Drop for FreeListAllocator {
    fn drop(&mut self) {
        let layout = Layout::from_size_align(self.capacity(), 8).unwrap();
        unsafe { dealloc(self.start, layout) };
    }
}
