use std::cmp::min;
use std::io::{self, IoSlice, Write};

pub struct Buffer {
    data: Vec<u8>,
    first: usize,
    last: usize,
}

impl Buffer {
    /// The buffer will hold capacity - 1 elements since one memory location is used a sentinel.
    pub fn new(capacity: usize) -> Self {
        let first = capacity / 2;
        let last = first;
        Self {
            data: vec![0; capacity],
            first,
            last,
        }
    }

    fn drop_n_front(&mut self, count: usize) {
        if self.first <= self.last {
            let len = min(self.last - self.first, count);
            self.first += len;
        }
        else {
            let len = min(self.data.len() - self.first, count);
            if len < count {
                let len = min(self.last, count - len);
                self.first = len;
            }
            else {
                self.first = (self.first + len) % self.data.len();
            }
        }
    }

    /// Returns the number of elements that were copied into the buffer.
    pub fn extend_back(&mut self, elements: &[u8]) -> usize {
        if self.last < self.first {
            let len = min(self.first - self.last - 1, elements.len()); // NOTE: minus one for the sentinel.
            let end = self.last + len;
            self.data[self.last..end].copy_from_slice(&elements[..len]);
            len
        }
        else {
            let mut len = min(self.data.len() - self.last, elements.len());
            let mut end = self.last + len;
            self.data[self.last..end].copy_from_slice(&elements[..len]);
            if len < elements.len() {
                end = min(elements.len() - len, self.first - 1);
                let end2 = len + end;
                self.data[..end].copy_from_slice(&elements[len..end2]);
                len += end;
            }
            self.last = end;
            // TODO: copy to the start if needed.
            len
        }
    }

    #[inline]
    fn next_index(&self, index: usize) -> usize {
        (index + 1) % self.data.len()
    }

    pub fn pop_front(&mut self) -> Option<u8> {
        if self.first != self.last {
            let result = self.data[self.first];
            self.first = self.next_index(self.first);
            Some(result)
        }
        else {
            None
        }
    }

    pub fn push_back(&mut self, element: u8) -> bool {
        let index = self.next_index(self.last);
        if index != self.first {
            self.data[self.last] = element;
            self.last = index;
            true
        }
        else {
            false
        }
    }

    pub fn write_to<W: Write>(&mut self, stream: &mut W) -> io::Result<()> {
        let size =
            if self.first == self.last {
                return Ok(());
            }
            else if self.first < self.last {
                stream.write_vectored(&[IoSlice::new(&self.data[self.first..self.last])])?
            }
            else {
                stream.write_vectored(&[
                    IoSlice::new(&self.data[self.first..]),
                    IoSlice::new(&self.data[..self.last]),
                ])?
            };
        self.drop_n_front(size);
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::Buffer;

    #[test]
    fn test_buffer() {
        let mut buffer = Buffer::new(10);
        assert!(buffer.push_back(1));
        assert!(buffer.push_back(2));
        assert!(buffer.push_back(3));

        assert_eq!(buffer.pop_front(), Some(1));
        assert_eq!(buffer.pop_front(), Some(2));
        assert_eq!(buffer.pop_front(), Some(3));
        assert_eq!(buffer.pop_front(), None);

        for i in 1..10 {
            assert!(buffer.push_back(i));
        }

        assert!(!buffer.push_back(10));

        for i in 1..10 {
            assert_eq!(buffer.pop_front(), Some(i));
        }

        assert_eq!(buffer.pop_front(), None);

        assert!(buffer.push_back(1));
        assert_eq!(buffer.pop_front(), Some(1));
        assert_eq!(buffer.pop_front(), None);
        assert!(buffer.push_back(2));
        assert_eq!(buffer.pop_front(), Some(2));
        assert_eq!(buffer.pop_front(), None);
        assert!(buffer.push_back(3));
        assert_eq!(buffer.pop_front(), Some(3));
        assert_eq!(buffer.pop_front(), None);

        assert_eq!(buffer.extend_back(&[1, 2, 3]), 3);
        assert_eq!(buffer.pop_front(), Some(1));
        assert_eq!(buffer.pop_front(), Some(2));
        assert_eq!(buffer.pop_front(), Some(3));
        assert_eq!(buffer.pop_front(), None);

        assert_eq!(buffer.extend_back(&[1, 2, 3, 4, 5, 6, 7, 8, 9]), 9);

        for i in 1..10 {
            assert_eq!(buffer.pop_front(), Some(i));
        }

        assert_eq!(buffer.pop_front(), None);

        assert_eq!(buffer.extend_back(&[1]), 1);
        assert_eq!(buffer.extend_back(&[2]), 1);
        assert_eq!(buffer.extend_back(&[3]), 1);

        assert_eq!(buffer.pop_front(), Some(1));
        assert_eq!(buffer.pop_front(), Some(2));
        assert_eq!(buffer.pop_front(), Some(3));
        assert_eq!(buffer.pop_front(), None);

        assert_eq!(buffer.extend_back(&[1, 2, 3]), 3);
        assert_eq!(buffer.extend_back(&[1, 2, 3, 4, 5, 6, 7, 8, 9]), 6);

        assert_eq!(buffer.pop_front(), Some(1));
        assert_eq!(buffer.pop_front(), Some(2));
        assert_eq!(buffer.pop_front(), Some(3));

        for i in 1..=6 {
            assert_eq!(buffer.pop_front(), Some(i));
        }

        assert_eq!(buffer.pop_front(), None);

        assert_eq!(buffer.extend_back(&[1, 2, 3, 4, 5, 6, 7, 8, 9]), 9);
        buffer.drop_n_front(3);

        for i in 4..=9 {
            assert_eq!(buffer.pop_front(), Some(i));
        }

        assert_eq!(buffer.pop_front(), None);

        assert_eq!(buffer.extend_back(&[1, 2, 3, 4, 5, 6, 7, 8, 9]), 9);
        buffer.drop_n_front(6);

        for i in 7..=9 {
            assert_eq!(buffer.pop_front(), Some(i));
        }

        assert_eq!(buffer.pop_front(), None);

        assert_eq!(buffer.extend_back(&[1, 2, 3, 4, 5, 6, 7, 8, 9]), 9);
        buffer.drop_n_front(8);

        assert_eq!(buffer.pop_front(), Some(9));
        assert_eq!(buffer.pop_front(), None);

        assert_eq!(buffer.extend_back(&[1, 2, 3, 4, 5, 6, 7, 8, 9]), 9);
        for _ in 0..5 {
            buffer.drop_n_front(1);
        }

        for i in 6..=9 {
            assert_eq!(buffer.pop_front(), Some(i));
        }

        assert_eq!(buffer.pop_front(), None);

        // TODO: test drop_n_front(0) and extend_back(&[]).
    }

    #[test]
    fn test_buffer_write() {
        let mut buffer = Buffer::new(10);
        assert_eq!(buffer.extend_back(&[1, 2, 3, 4, 5, 6, 7, 8, 9]), 9);
        let mut vector = vec![];
        buffer.write_to(&mut vector);
        assert_eq!(vector, &[1, 2, 3, 4, 5, 6, 7, 8, 9]);
    }
}
