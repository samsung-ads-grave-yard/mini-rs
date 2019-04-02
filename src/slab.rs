pub struct Entry(usize);

impl Entry {
    pub fn from(index: usize) -> Self {
        Self(index)
    }

    pub fn index(&self) -> usize {
        self.0
    }
}

enum Node<T> {
    Empty,
    Occupied(T),
    Reserved,
}

pub struct Slab<T> {
    data: Vec<Node<T>>, // TODO: optimize this.
}

impl<T> Slab<T> {
    pub fn new() -> Self {
        Self {
            data: vec![],
        }
    }

    pub fn get(&self, entry: Entry) -> Option<&T> {
        None
    }

    pub fn insert(&mut self, value: T) -> Entry {
        Entry(0)
    }

    pub fn remove(&mut self, entry: Entry) {
    }

    pub fn reserve_entry(&mut self) -> Entry {
    }

    pub fn set(&mut self, entry: Entry, value: T) {
    }
}
