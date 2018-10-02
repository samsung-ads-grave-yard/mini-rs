// TODO: optimize this.
use std::mem;

pub type Entry = usize;

pub struct Slab<T> {
    data: Vec<Option<T>>,
}

impl<T> Slab<T> {
    pub fn new() -> Self {
        Self {
            data: vec![],
        }
    }

    pub fn entry(&mut self) -> Entry {
        for (index, element) in self.data.iter().enumerate() {
            if element.is_none() {
                return index;
            }
        }
        let index = self.data.len();
        self.data.push(None);
        index
    }

    pub fn get(&self, index: Entry) -> Option<&T> {
        self.data[index].as_ref()
    }

    pub fn get_mut(&mut self, index: Entry) -> Option<&mut T> {
        self.data[index].as_mut()
    }

    pub fn insert(&mut self, index: Entry, value: T) {
        self.data[index] = Some(value);
    }

    pub fn remove(&mut self, index: Entry) -> Option<T> {
        let result = mem::replace(&mut self.data[index], None);
        result
    }
}
