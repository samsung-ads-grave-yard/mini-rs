use std::cmp::min;
use std::io::{
    self,
    IoSlice,
    IoSliceMut,
    Read,
    Write,
};

pub struct Buffer {
    data: Vec<u8>,
    read_index: usize,
    write_index: usize,
}

impl Buffer {
    pub fn new(capacity: usize) -> Self {
        assert!(capacity.is_power_of_two());
        let read_index = capacity / 2;
        let write_index = read_index;
        Self {
            data: vec![0; capacity],
            read_index,
            write_index,
        }
    }

    pub fn drain_to_vec(&mut self) -> Vec<u8> {
        let mut result = vec![0; self.size()];
        let read_index = self.mask(self.read_index);
        let write_index = self.mask(self.write_index);
        if read_index < write_index {
            println!("Size: {}: {} {}", self.size(), write_index, read_index);
            result.copy_from_slice(&self.data[read_index..write_index]);
        }
        else {
            let end = self.data.capacity() - read_index;
            &mut result[..end].copy_from_slice(&self.data[read_index..]);
            &mut result[end..].copy_from_slice(&self.data[..write_index]);
        }
        self.read_index = self.data.capacity() / 2;
        self.write_index = self.read_index;
        result
    }

    pub fn drop_n_front(&mut self, count: usize) {
        self.read_index = self.read_index.wrapping_add(min(self.size(), count));
    }

    /// Returns the number of elements that were copied into the buffer.
    pub fn extend_back(&mut self, elements: &[u8]) -> usize {
        // TODO: if elements.is_empty(), return 0 immediately?
        let space_left = self.data.capacity() - self.size();
        let count = elements.len();
        let insert_count = min(count, space_left);
        let write_index = self.mask(self.write_index.wrapping_add(1));
        let end_length = min(self.data.capacity() - write_index, insert_count);
        let end = write_index + end_length;
        self.data[write_index..end].copy_from_slice(&elements[..end_length]);
        if insert_count > end_length {
            let start_length = insert_count - end_length;
            self.data[..start_length].copy_from_slice(&elements[end_length..insert_count]);
        }
        self.write_index = self.write_index.wrapping_add(insert_count);
        insert_count
    }

    pub fn is_empty(&self) -> bool {
        self.read_index == self.write_index
    }

    pub fn is_full(&self) -> bool {
        self.size() == self.data.capacity()
    }

    fn mask(&self, index: usize) -> usize {
        index & (self.data.capacity() - 1)
    }

    pub fn pop_front(&mut self) -> Option<u8> {
        if self.is_empty() {
            return None;
        }
        self.read_index = self.read_index.wrapping_add(1);
        let index = self.mask(self.read_index);
        Some(self.data[index])
    }

    pub fn push_back(&mut self, element: u8) -> bool {
        if self.is_full() {
            return false;
        }
        self.write_index = self.write_index.wrapping_add(1);
        let index = self.mask(self.write_index);
        self.data[index] = element;
        true
    }

    pub fn read_from<R: Read>(&mut self, stream: &mut R) -> io::Result<usize> {
        let masked_read = self.mask(self.read_index.wrapping_add(1));
        let masked_write = self.mask(self.write_index.wrapping_add(1));
        // TODO: special case for empty buffer?
        let size =
            if masked_write < masked_read {
                stream.read(&mut self.data[masked_write..masked_read])?
            }
            else {
                let (start, end) = self.data.split_at_mut(masked_read);
                let start_index = masked_write - masked_read;
                stream.read_vectored(&mut [
                    IoSliceMut::new(&mut end[start_index..]),
                    IoSliceMut::new(start),
                ])?
            };
        self.write_index = self.write_index.wrapping_add(size);
        Ok(size)
    }

    pub fn write_to<W: Write>(&mut self, stream: &mut W) -> io::Result<()> {
        let masked_read = self.mask(self.read_index.wrapping_add(1));
        let masked_write = self.mask(self.write_index.wrapping_add(1));
        let size =
            if self.is_empty() {
                return Ok(());
            }
            else if masked_read < masked_write {
                stream.write(&self.data[masked_read..masked_write])?
            }
            else {
                stream.write_vectored(&[
                    IoSlice::new(&self.data[masked_read..]),
                    IoSlice::new(&self.data[..masked_write]),
                ])?
            };
        self.drop_n_front(size);
        Ok(())
    }

    pub fn size(&self) -> usize {
        self.write_index - self.read_index
    }
}

#[cfg(test)]
mod tests {
    use std::io::Cursor;

    use super::Buffer;

    #[test]
    fn test_buffer() {
        let mut buffer = Buffer::new(16);
        assert!(buffer.push_back(1));
        assert!(buffer.push_back(2));
        assert!(buffer.push_back(3));

        assert_eq!(buffer.pop_front(), Some(1));
        assert_eq!(buffer.pop_front(), Some(2));
        assert_eq!(buffer.pop_front(), Some(3));
        assert_eq!(buffer.pop_front(), None);

        for i in 0..16 {
            assert!(buffer.push_back(i));
        }

        assert!(!buffer.push_back(16));

        for i in 0..16 {
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

        assert_eq!(buffer.extend_back(&[1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15]), 15);

        for i in 1..16 {
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
        assert_eq!(buffer.extend_back(&[1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15]), 13);

        assert_eq!(buffer.pop_front(), Some(1));
        assert_eq!(buffer.pop_front(), Some(2));
        assert_eq!(buffer.pop_front(), Some(3));

        for i in 1..=13 {
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

        assert_eq!(buffer.extend_back(&[]), 0);
        assert_eq!(buffer.pop_front(), None);

        buffer.drop_n_front(0);
        assert_eq!(buffer.pop_front(), None);

        assert_eq!(buffer.extend_back(&[1]), 1);
        assert_eq!(buffer.extend_back(&[]), 0);
        buffer.drop_n_front(0);
        assert_eq!(buffer.pop_front(), Some(1));
        assert_eq!(buffer.pop_front(), None);
    }

    #[test]
    fn test_buffer_write() {
        let mut buffer = Buffer::new(16);
        assert_eq!(buffer.extend_back(&[0, 1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15]), 16);
        let mut vector = vec![];
        buffer.write_to(&mut vector).expect("write to");
        assert_eq!(vector, &[0, 1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15]);
        assert!(buffer.is_empty());

        let mut vector = vec![];
        buffer.write_to(&mut vector).expect("write to");
        assert_eq!(vector, &[]);

        assert!(buffer.push_back(1));
        buffer.write_to(&mut vector).expect("write to");
        assert_eq!(vector, &[1]);

        let mut vector = vec![];
        assert!(buffer.push_back(1));
        assert!(buffer.push_back(2));
        buffer.write_to(&mut vector).expect("write to");
        assert_eq!(vector, &[1, 2]);
    }

    #[test]
    fn test_buffer_read() {
        let mut buffer = Buffer::new(16);
        buffer.read_from(&mut Cursor::new(vec![1, 2, 3])).expect("read from");

        for i in 1..=3 {
            assert_eq!(buffer.pop_front(), Some(i));
        }
        assert_eq!(buffer.pop_front(), None);

        buffer.read_from(&mut Cursor::new(vec![1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15])).expect("read from");
        for i in 1..=15 {
            assert_eq!(buffer.pop_front(), Some(i));
        }
        assert_eq!(buffer.pop_front(), None);

        buffer.read_from(&mut Cursor::new(vec![1, 2, 3, 4, 5, 6, 7, 8])).expect("read from");
        for i in 1..=8 {
            assert_eq!(buffer.pop_front(), Some(i));
        }
        assert_eq!(buffer.pop_front(), None);

        buffer.read_from(&mut Cursor::new(vec![1, 2, 3, 4, 5])).expect("read from");
        for i in 1..=5 {
            assert_eq!(buffer.pop_front(), Some(i));
        }
        assert_eq!(buffer.pop_front(), None);

        buffer.read_from(&mut Cursor::new(vec![1, 2, 3])).expect("read from");
        for i in 1..=3 {
            assert_eq!(buffer.pop_front(), Some(i));
        }
        assert_eq!(buffer.pop_front(), None);

        buffer.read_from(&mut Cursor::new(vec![1, 2, 3, 4, 5, 6, 7])).expect("read from");
        for i in 1..=7 {
            assert_eq!(buffer.pop_front(), Some(i));
        }
        assert_eq!(buffer.pop_front(), None);
    }
}
