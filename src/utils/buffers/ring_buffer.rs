use len_trait::len::{Empty, Len};
use std::fmt::Debug;
use std::fmt::Formatter;
use std::ops::{Deref, DerefMut, Index, IndexMut};

/// A buffer that stores elements of type `T` in a First-In-First-Out manner.
///
/// The `RingBuffer` struct allows enqueueing new elements onto its tail. When
/// an element is enqueued, all older elements shift towards the head and the
/// oldest element is popped off the head of the buffer. Due to its fixed-size
/// nature, the ring buffer is very fast and doesn't dynamically reallocate
/// itself, or move any elements around when an element is added.
///
/// It is foundational to visualizers, certain audio effects, and many other
/// real-time applications where elements need to be sequentally enqueued.
///
/// # Example
///
/// ```
/// use cyma::utils::RingBuffer;
///
/// let mut rb = RingBuffer::<i32>::new(4);
///
/// rb.enqueue(1);
/// rb.enqueue(2);
/// dbg!(&rb);
///
/// // &rb = [0, 0, 1, 2]
///
/// rb.enqueue(3);
/// rb.enqueue(4);
/// dbg!(&rb);
///
/// // &rb = [1, 2, 3, 4]
///
/// rb.enqueue(5);
/// dbg!(&rb);
///
/// // &rb = [2, 3, 4, 5]
///
/// let tripled: Vec<i32> = (&rb).into_iter().map(|x| *x * 3).collect();
///
/// dbg!(tripled);
///
/// // tripled = [6, 9, 12, 15]
/// ```
///
/// Internally, this buffer stores elements in sequence, wrapping around to
/// replace old values. The order in which elements are retrieved by indexing
/// into a RingBuffer, or iterating over it, thus differs from the way they're
/// internally stored. If you dereference this type, you get a slice with the
/// internal order of all elements.
///
/// ```
/// use cyma::utils::RingBuffer;
///
/// let mut rb = RingBuffer::<i32>::new(4);
///
/// rb.enqueue(1);
/// rb.enqueue(2);
/// rb.enqueue(3);
/// rb.enqueue(4);
/// rb.enqueue(5);
///
/// assert_eq!(rb[0], 2);       //  rb = [2, 3, 4, 5]
/// assert_eq!((*rb)[0], 5);    // *rb = [5, 2, 3, 4]
/// ```
#[derive(Clone, PartialEq, Eq, Default, Hash)]
pub struct RingBuffer<T> {
    head: usize,
    size: usize,
    data: Vec<T>,
}

impl<T: Default + Copy + Debug> RingBuffer<T> {
    /// Constructs a new RingBuffer with the given size.
    pub fn new(size: usize) -> Self {
        Self {
            head: 0,
            size,
            data: vec![T::default(); size],
        }
    }

    /// Resizes the RingBuffer to the given size.
    ///
    /// Internally, this either calls [`shrink()`](`RingBuffer::shrink()`), or
    /// [`grow()`](`RingBuffer::grow()`), depending on the desired size. This
    /// operation keeps the order of the values intact.
    pub fn resize(self: &mut Self, size: usize) {
        if size < self.size {
            self.shrink(size);
        } else if size > self.size {
            self.grow(size);
        }
    }

    /// Shrinks the RingBuffer to the given size.
    ///
    /// The most recently enqueued elements are preserved. This operation keeps
    /// the order of the values intact.
    pub fn shrink(self: &mut Self, size: usize) {
        let mut data = vec![];

        if size <= self.head {
            // Copy the last `size` elements before the head
            data.extend_from_slice(&self.data[self.head - size..self.head]);
        } else {
            // Copy the last `size` elements before the buffer wraps around
            data.extend_from_slice(&self.data[self.size - (size - self.head)..self.size]);
            // Copy everything before the head
            data.extend_from_slice(&self.data[0..self.head]);
        }

        self.head = 0;
        self.size = size;
        self.data = data;
    }

    /// Grows the RingBuffer.
    ///
    /// The extra space is filled with the default values for your data type
    /// (usually 0). This operation keeps the order of the values intact.
    pub fn grow(self: &mut Self, size: usize) {
        let mut data = vec![];

        // Copy everything after the head
        data.extend_from_slice(&self.data[self.head..self.size]);
        // Copy everything before the head
        data.extend_from_slice(&self.data[0..self.head]);

        for _ in self.size..size {
            data.push(T::default());
        }

        self.data = data;
        self.head = self.size;
        self.size = size;
    }

    /// Enqueues an element into the RingBuffer.
    ///
    /// Once enqueued, the value is situated at the tail of the buffer and the
    /// oldest element is removed from the head.
    pub fn enqueue(self: &mut Self, value: T) {
        self.data[self.head] = value;
        self.head = (self.head + 1) % self.size;
    }

    /// Clears the entire buffer, filling it with default values (usually 0)
    pub fn clear(self: &mut Self) {
        self.data.iter_mut().for_each(|x| *x = T::default());
    }
}

impl<T: Debug + Copy> Debug for RingBuffer<T> {
    fn fmt(&self, f: &mut Formatter) -> Result<(), std::fmt::Error> {
        f.debug_list().entries(self.into_iter()).finish()
    }
}

impl<'a, T: Copy> IntoIterator for &'a RingBuffer<T> {
    type Item = &'a T;
    type IntoIter = Iter<'a, T>;

    /// Creates an iterator from a reference.
    fn into_iter(self) -> Self::IntoIter {
        Iter {
            ring_buffer: self,
            index: 0,
            index_back: self.size,
        }
    }
}

impl<T> Empty for RingBuffer<T> {
    fn is_empty(self: &Self) -> bool {
        self.size == 0
    }
}
impl<T> Len for RingBuffer<T> {
    /// Returns the length of the buffer.
    fn len(self: &Self) -> usize {
        self.size
    }
}

impl<T> Index<usize> for RingBuffer<T> {
    type Output = T;

    fn index(&self, index: usize) -> &Self::Output {
        if index >= self.size {
            panic!(
                "Invalid ring buffer access: Index {} is out of range for ring buffer of size {}",
                index, self.size
            );
        }
        &self.data[(self.head + index) % self.size]
    }
}
impl<T> IndexMut<usize> for RingBuffer<T> {
    fn index_mut(&mut self, index: usize) -> &mut Self::Output {
        if index >= self.size {
            panic!(
                "Invalid ring buffer access: Index {} is out of range for ring buffer of size {}",
                index, self.size
            );
        }
        &mut self.data[(self.head + index) % self.size]
    }
}

/// An iterator that moves out of a [`RingBuffer`].
///
/// This struct is created by the `into_iter` method on [`RingBuffer`]
pub struct IntoIter<T> {
    ring_buffer: RingBuffer<T>,
    index: usize,
    index_back: usize,
}
impl<T: Copy> Iterator for IntoIter<T> {
    type Item = T;
    fn next(&mut self) -> Option<Self::Item> {
        if self.index >= self.ring_buffer.size {
            return None;
        };
        let item = Some(self.ring_buffer[self.index]);
        self.index += 1;
        item
    }
}
impl<T: Copy> DoubleEndedIterator for IntoIter<T> {
    fn next_back(&mut self) -> Option<Self::Item> {
        if self.index_back == 0 {
            return None;
        }
        self.index_back -= 1;
        Some(self.ring_buffer[self.index_back])
    }
}

/// An immutable iterator over a [`RingBuffer`].
///
/// This struct is created by the `into_iter` method on [`RingBuffer`]
pub struct Iter<'a, T> {
    ring_buffer: &'a RingBuffer<T>,
    index: usize,
    index_back: usize,
}
impl<'a, T: Copy> Iterator for Iter<'a, T> {
    type Item = &'a T;
    fn next(&mut self) -> Option<Self::Item> {
        if self.index >= self.ring_buffer.size {
            return None;
        };
        let item = Some(&self.ring_buffer[self.index]);
        self.index += 1;
        item
    }
}
impl<'a, T: Copy> DoubleEndedIterator for Iter<'a, T> {
    fn next_back(&mut self) -> Option<Self::Item> {
        if self.index_back == 0 {
            return None;
        }
        self.index_back -= 1;
        Some(&self.ring_buffer[self.index_back])
    }
}

impl<T: Default> Deref for RingBuffer<T> {
    type Target = [T];
    /// Dereferences the underlying data, giving you direct access to it.
    ///
    /// Crucially, this does not preserve the ordering you would get by
    /// iterating over the `RingBuffer` or indexing it directly.
    fn deref(&self) -> &Self::Target {
        self.data.deref()
    }
}
impl<T: Default> DerefMut for RingBuffer<T> {
    /// Mutably dereferences the underlying data, giving you direct access to
    /// it.
    ///
    /// Crucially, this does not preserve the ordering you would get by
    /// iterating over the `RingBuffer` or indexing it directly.
    fn deref_mut(&mut self) -> &mut Self::Target {
        self.data.deref_mut()
    }
}

#[cfg(test)]
mod tests {
    use super::RingBuffer;

    #[test]
    fn basics() {
        let mut rb = RingBuffer::<i32>::new(4);

        // Is the buffer filled with zeroes?
        assert_eq!(rb.data, vec![0; 4]);

        rb.enqueue(1);
        rb.enqueue(2);
        rb.enqueue(3);

        // Is the value at the tail (before the head) equal to 0?
        assert_eq!(rb.data[(rb.head + rb.size - 1) % rb.size], 3);

        // Is the value at the head equal to 0?
        assert_eq!(rb.data[rb.head], 0);

        rb.enqueue(4);
        rb.enqueue(5);
        rb.enqueue(6);

        // Have the earlier values been overwritten?
        assert!(!rb.data.contains(&1));
        assert!(!rb.data.contains(&2));

        // Do the last 4 values exist?
        assert!(rb.data.contains(&3));
        assert!(rb.data.contains(&4));
        assert!(rb.data.contains(&5));
        assert!(rb.data.contains(&6));
    }

    #[test]
    fn clear() {
        let mut rb = RingBuffer::<i32>::new(4);

        rb.enqueue(1);
        rb.enqueue(2);
        rb.enqueue(3);

        assert_ne!(rb.data, vec![0; 4]);

        rb.clear();

        assert_eq!(rb.data, vec![0; 4]);
    }

    #[test]
    fn resize() {
        let mut rb = RingBuffer::<i32>::new(4);
        rb.enqueue(1);
        rb.enqueue(2);
        rb.enqueue(3);
        rb.enqueue(4);
        rb.enqueue(5);
        rb.enqueue(6);

        // Growing RingBuffers
        {
            let mut rb_grown = rb.clone();
            rb_grown.grow(6);
            let mut rb_resized = rb.clone();
            rb_resized.resize(6);
            // Is the last inserted datum the same?
            assert_eq!(
                rb_grown.data[(rb_grown.head + rb_grown.size - 1) % rb_grown.size],
                rb_resized.data[(rb_resized.head + rb_resized.size - 1) % rb_resized.size]
            );
            // Is the buffer zero-padded?
            assert_eq!(rb_grown.data[rb_grown.head], 0);
            // Does resize() do the same thing as grow() here?
            assert_eq!(rb_grown, rb_resized);
        }

        // Shrinking RingBuffers
        {
            let mut rb_shrunk = rb.clone();
            rb_shrunk.shrink(3);
            let mut rb_resized = rb.clone();
            rb_resized.resize(3);
            // Is the last inserted datum the same?
            assert_eq!(
                rb_shrunk.data[(rb_shrunk.head + rb_shrunk.size - 1) % rb_shrunk.size],
                rb_resized.data[(rb_resized.head + rb_resized.size - 1) % rb_resized.size]
            );
            // Does resize() do the same thing as grow() here?
            assert_eq!(rb_shrunk, rb_resized);
        }
    }

    #[test]
    fn indexing() {
        let mut rb = RingBuffer::<i32>::new(4);

        rb.enqueue(1);
        rb.enqueue(2);
        rb.enqueue(3);

        // Is the last value still equal to 0?
        assert_eq!(rb[0], 0);

        // Were the first bunch of values inserted correctly?
        assert_eq!(rb[1], 1);
        assert_eq!(rb[2], 2);
        assert_eq!(rb[3], 3);

        rb[1] *= 2;
        rb[2] *= 3;
        rb[3] *= 4;

        // Were the values multiplied correctly?
        assert_eq!(rb[1], 2);
        assert_eq!(rb[2], 6);
        assert_eq!(rb[3], 12);

        rb.enqueue(4);
        rb.enqueue(5);

        // Have the newer values "pushed back" the older ones?
        assert_eq!(rb[0], 6);
        assert_eq!(rb[1], 12);
        assert_eq!(rb[2], 4);
        assert_eq!(rb[3], 5);

        // Can we set an element of the buffer?
        rb[2] = 10;
        assert_eq!(rb[2], 10);
    }

    #[test]
    #[should_panic]
    fn invalid_access() {
        let mut rb = RingBuffer::<i32>::new(4);

        rb.enqueue(1);
        rb.enqueue(2);
        rb.enqueue(3);

        rb[4];
    }

    #[test]
    fn iteration() {
        let mut rb = RingBuffer::<i32>::new(4);

        rb.enqueue(7);
        rb.enqueue(3);
        rb.enqueue(8);

        // Does iteration give us the data in the correct order?
        let mut iter = (&rb).into_iter();
        assert_eq!(iter.next(), Some(&0));
        assert_eq!(iter.next(), Some(&7));
        assert_eq!(iter.next(), Some(&3));
        assert_eq!(iter.next(), Some(&8));
        // Do we get `None` once we step out of bounds?
        assert_eq!(iter.next(), None);

        // Is enumeration analogous to indexing?
        (&rb)
            .into_iter()
            .enumerate()
            .for_each(|(i, n)| assert_eq!(n, &rb[i]));

        // Can we sum up everything?
        let sum: i32 = (&rb).into_iter().sum();
        assert_eq!(sum, 18);

        // Can we properly collect the buffer into a vector?
        let vec: Vec<i32> = (&rb).into_iter().copied().collect();
        assert_eq!(vec, vec![0, 7, 3, 8]);

        // Can we triple each element?
        let tripled: Vec<i32> = (&rb).into_iter().map(|x| *x * 3).collect();
        assert_eq!(tripled, vec![0, 21, 9, 24]);

        rb.enqueue(1);
        rb.enqueue(2);

        // Do we still get our data in the correct order?
        let mut iter = rb.into_iter();
        assert_eq!(iter.next(), Some(&3));
        assert_eq!(iter.next(), Some(&8));
        assert_eq!(iter.next(), Some(&1));
        assert_eq!(iter.next(), Some(&2));
        // Do we still get `None` once we step out of bounds?
        assert_eq!(iter.next(), None);
    }

    #[test]
    fn deref() {
        let mut rb = RingBuffer::<i32>::new(4);

        rb.enqueue(7);
        rb.enqueue(3);
        rb.enqueue(8);
        rb.enqueue(1);
        rb.enqueue(9);

        // Can we get a value by dereferencing it?
        assert_eq!((*rb)[0], 9);

        // Can we set a value by mutably dereferencing it?
        (*rb)[2] = 200;
        assert_eq!(rb[1], 200);
    }
}
