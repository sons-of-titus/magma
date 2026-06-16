/// A simple reusable ID allocator using a free list.
/// IDs are never reused immediately — they go through a free list
/// to avoid confusion with stale references.
#[derive(Debug, Clone)]
pub struct IdPool {
    next: u64,
    free: Vec<u64>,
}

impl IdPool {
    pub fn new(start: u64) -> Self {
        IdPool {
            next: start,
            free: Vec::new(),
        }
    }

    pub fn allocate(&mut self) -> u64 {
        match self.free.pop() {
            Some(id) => id,
            None => {
                let id = self.next;
                self.next += 1;
                id
            }
        }
    }

    pub fn release(&mut self, id: u64) {
        self.free.push(id);
    }
}
