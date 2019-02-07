use std::ptr;
use std::sync::atomic::{
    AtomicPtr,
    AtomicUsize,
    Ordering,
};

struct Node<T> {
    sequence: AtomicUsize,
    //data: AtomicUsize, // TODO: use AtomicUsize and a vec containing the elements.
    data: AtomicPtr<T>,
}

impl<T> Node<T> {
    fn new(sequence: usize) -> Self {
        Self {
            sequence: AtomicUsize::new(sequence),
            data: AtomicPtr::new(ptr::null_mut()),
        }
    }
}

pub struct BoundedQueue<T> {
    first: AtomicUsize,
    last: AtomicUsize,
    count: AtomicUsize,
    capacity: usize,
    elements: Vec<Node<T>>,
}

impl<T> BoundedQueue<T> {
    pub fn new(capacity: usize) -> Self {
        assert!(capacity > 1, "BoundedQueue does not work with a capacity inferior to 2.");
        let mut elements = Vec::with_capacity(capacity);
        for i in 0..capacity {
            elements.push(Node::new(i));
        }
        Self {
            capacity,
            elements,
            first: AtomicUsize::new(0),
            last: AtomicUsize::new(0),
            count: AtomicUsize::new(0),
        }
    }

    #[inline(always)] // TODO: is it necessary?
    pub fn is_empty(&self) -> bool {
        self.count.load(Ordering::Acquire) == 0
    }

    pub fn pop(&self) -> Option<T> {
        let mut element;
        let mut first = self.first.load(Ordering::Acquire);

        loop {
            element = &self.elements[first % self.capacity];
            let sequence = element.sequence.load(Ordering::Acquire);
            let diff = sequence as isize - (first + 1) as isize;
            if diff == 0 && self.first.compare_exchange_weak(first, first + 1, Ordering::SeqCst, Ordering::SeqCst).is_ok() {
                break;
            }
            else if diff < 0 {
                return None;
            }

            first = self.first.load(Ordering::Acquire);
        }

        let data = element.data.load(Ordering::Acquire);
        self.count.fetch_sub(1, Ordering::SeqCst);
        element.sequence.store(first + self.capacity, Ordering::Release);
        unsafe {
            Some(*Box::from_raw(data))
        }
    }

    pub fn push(&self, data: T) -> Result<(), T> {
        let mut last = self.last.load(Ordering::Acquire);
        let mut element;

        loop {
            element = &self.elements[last % self.capacity];
            let sequence = element.sequence.load(Ordering::Acquire);
            let diff = sequence as isize - last as isize;
            if diff == 0 && self.last.compare_exchange_weak(last, last + 1, Ordering::SeqCst, Ordering::SeqCst).is_ok() {
                break;
            }
            else if diff < 0 {
                return Err(data);
            }
            last = self.last.load(Ordering::Acquire);
        }

        // Past this point, any preemption will cause all other consumers to spin-lock waiting for
        // it to finish, IF AND ONLY IF they reach the end. Normal case: Producers are ahead
        // TODO: maybe do not use a Box here, but make T=Box<U> when needed.
        element.data.store(Box::into_raw(Box::new(data)), Ordering::Release);
        self.count.fetch_add(1, Ordering::SeqCst);
        element.sequence.store(last + 1, Ordering::Release);

        Ok(())
    }
}

impl<T> Drop for BoundedQueue<T> {
    fn drop(&mut self) {
        while self.pop().is_some() {
        }
    }
}

#[cfg(test)]
mod tests {
    use std::sync::{Arc, Mutex};
    use std::thread;
    use std::time::Duration;

    use super::BoundedQueue;

    #[test]
    fn test_single_thread() {
        let queue = BoundedQueue::new(3);
        queue.push(10).expect("push");
        assert_eq!(queue.pop(), Some(10));
        assert_eq!(queue.pop(), None);

        queue.push(11).expect("push");
        queue.push(12).expect("push");
        queue.push(13).expect("push");
        assert_eq!(queue.pop(), Some(11));
        assert_eq!(queue.pop(), Some(12));
        assert_eq!(queue.pop(), Some(13));
        assert_eq!(queue.pop(), None);

        queue.push(14).expect("push");
        queue.push(15).expect("push");
        assert_eq!(queue.pop(), Some(14));
        queue.push(16).expect("push");
        assert_eq!(queue.pop(), Some(15));
        assert_eq!(queue.pop(), Some(16));
        assert_eq!(queue.pop(), None);
    }

    #[test]
    fn test_multithread() {
        let queue = Arc::new(BoundedQueue::new(1_000_000));

        let results = Arc::new(Mutex::new(vec![]));

        let handle = {
            let queue = queue.clone();
            let results = results.clone();
            thread::spawn(move || {
                let mut elements = vec![];
                for _ in 0..50_000 {
                    loop {
                        if let Some(element) = queue.pop() {
                            elements.push(element);
                            //i += 1;
                            break;
                        }
                    }
                }
                thread::sleep(Duration::from_secs(1));
                for _ in 0..950_000 {
                    loop {
                        if let Some(element) = queue.pop() {
                            elements.push(element);
                            break;
                        }
                    }
                }
                *results.lock().expect("lock") = elements;
            })
        };

        let handle2 = {
            let queue = queue.clone();
            thread::spawn(move || {
                for i in 0..100_000 {
                    queue.push(i).expect("push");
                }
            })
        };

        let handle3 = {
            let queue = queue.clone();
            thread::spawn(move || {
                for i in 100_000..1_000_000 {
                    queue.push(i).expect("push");
                }
            })
        };

        handle.join().expect("join");
        handle2.join().expect("join");
        handle3.join().expect("join");

        let mut results = results.lock().expect("lock");
        assert_eq!(results.len(), 1_000_000);

        results.sort();

        for (i, &element) in results.iter().enumerate() {
            assert_eq!(element, i);
        }
    }

    #[test]
    fn test_multithread_mc() {
        let queue = Arc::new(BoundedQueue::new(1_000_000));

        let results = Arc::new(Mutex::new(vec![]));
        let results2 = Arc::new(Mutex::new(vec![]));

        let handle = {
            let queue = queue.clone();
            let results = results.clone();
            thread::spawn(move || {
                let mut elements = vec![];
                for _ in 0..50_000 {
                    loop {
                        if let Some(element) = queue.pop() {
                            elements.push(element);
                            break;
                        }
                    }
                }
                *results.lock().expect("lock") = elements;
            })
        };

        let handle4 = {
            let queue = queue.clone();
            let results = results2.clone();
            thread::spawn(move || {
                let mut elements = vec![];
                for _ in 0..950_000 {
                    loop {
                        if let Some(element) = queue.pop() {
                            elements.push(element);
                            break;
                        }
                    }
                }
                *results.lock().expect("lock") = elements;
            })
        };

        let handle2 = {
            let queue = queue.clone();
            thread::spawn(move || {
                for i in 0..100_000 {
                    queue.push(i).expect("push");
                }
            })
        };

        let handle3 = {
            let queue = queue.clone();
            thread::spawn(move || {
                for i in 100_000..1_000_000 {
                    queue.push(i).expect("push");
                }
            })
        };

        handle.join().expect("join");
        handle2.join().expect("join");
        handle3.join().expect("join");
        handle4.join().expect("join");

        let mut results = results.lock().expect("lock");
        let mut results2 = results2.lock().expect("lock");
        assert_eq!(results.len() + results2.len(), 1_000_000);

        results.append(&mut results2);
        results.sort();

        for (i, &element) in results.iter().enumerate() {
            assert_eq!(element, i);
        }
    }

    #[test]
    #[ignore]
    fn test_underflow() {
        let queue = Arc::new(BoundedQueue::new(2));

        let q = queue.clone();
        thread::spawn(move || {
            for _ in 0..5_000_000_000i64 {
                while q.pop().is_none() {
                }
                while q.pop().is_none() {
                }
            }
        });

        for _ in 0..5_000_000_000i64 {
            while queue.push(10).is_err() {
            }
            while queue.push(10).is_err() {
            }
        }
    }
}
