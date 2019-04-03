use std::mem;

use self::Node::*;

#[derive(Clone, Copy)]
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

impl<T> Node<T> {
    fn as_mut_option(&mut self) -> Option<&mut T> {
        match self {
            Empty | Reserved => None,
            Occupied(ref mut value) => Some(value),
        }
    }

    fn as_option(&self) -> Option<&T> {
        match self {
            Empty | Reserved => None,
            Occupied(ref value) => Some(value),
        }
    }

    fn into_option(self) -> Option<T> {
        match self {
            Empty | Reserved => None,
            Occupied(value) => Some(value),
        }
    }
}

pub struct Slab<T> {
    elements: Vec<Node<T>>, // TODO: optimize this.
}

impl<T> Slab<T> {
    pub fn new() -> Self {
        Self {
            elements: vec![],
        }
    }

    pub fn capacity(&self) -> usize {
        self.elements.len()
    }

    pub fn get(&self, Entry(index): Entry) -> Option<&T> {
        self.elements[index].as_option()
    }

    pub fn get_mut(&mut self, Entry(index): Entry) -> Option<&mut T> {
        self.elements[index].as_mut_option()
    }

    pub fn insert(&mut self, value: T) -> Entry {
        let entry = self.reserve_entry();
        self.set(entry, value);
        entry
    }

    pub fn remove(&mut self, Entry(index): Entry) -> Option<T> {
        let value = mem::replace(&mut self.elements[index], Empty);
        value.into_option()
    }

    pub fn reserve_entry(&mut self) -> Entry {
        for (index, element) in self.elements.iter().enumerate() {
            if let Empty = element {
                return Entry(index);
            }
        }
        let index = self.elements.len();
        self.elements.push(Empty);
        Entry(index)
    }

    pub fn reserve_remove(&mut self, Entry(index): Entry) -> Option<T> {
        let value = mem::replace(&mut self.elements[index], Reserved);
        value.into_option()
    }

    pub fn set(&mut self, Entry(index): Entry, value: T) {
        self.elements[index] = Occupied(value);
    }
}

#[cfg(test)]
mod tests {
    #[test]
    fn slab() {
    }
}
