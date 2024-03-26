use len_trait::len::{Empty, Len};
use nih_plug::buffer::Buffer;
use num_traits::real::Real;
use std::{
    fmt::{Debug, Formatter},
    ops::{Deref, DerefMut, Index, IndexMut},
};

use super::{ring_buffer::Iter, RingBuffer};

#[derive(Clone, PartialEq, Default)]
pub struct PeakBuffer<T> {
    buffer: RingBuffer<T>,
    // Minimum and maximum accumulators
    max_acc: T,
    // The gap between elements of the buffer in samples
    sample_delta: f32,
    // Used to calculate the sample_delta
    sample_rate: f32,
    duration: f32,
    // The current time, counts down from sample_delta to 0
    t: f32,
}

impl<T> PeakBuffer<T>
where
    T: Clone + Copy + Default + Debug + PartialOrd + Real,
{
    pub fn new(size: usize, sample_rate: f32, duration: f32) -> Self {
        let sample_delta = Self::sample_delta(size, sample_rate as f32, duration as f32);
        Self {
            buffer: RingBuffer::<T>::new(size),
            max_acc: T::default(),
            sample_delta,
            sample_rate,
            duration,
            t: sample_delta,
        }
    }

    // TODO: Allow resizing without clearing
    pub fn resize(self: &mut Self, size: usize) {
        if self.buffer.len() == size {
            return;
        };
        self.buffer.resize(size);
        self.sample_delta = Self::sample_delta(size, self.sample_rate, self.duration);
        self.buffer.clear();
    }

    pub fn set_sample_rate(self: &mut Self, sample_rate: f32) {
        self.sample_rate = sample_rate;
        self.sample_delta = Self::sample_delta(self.buffer.len(), sample_rate, self.duration);
        self.buffer.clear();
    }

    pub fn set_duration(self: &mut Self, duration: f32) {
        self.duration = duration;
        self.sample_delta = Self::sample_delta(self.buffer.len(), self.sample_rate, duration);
        self.buffer.clear();
    }

    fn sample_delta(size: usize, sample_rate: f32, duration: f32) -> f32 {
        (sample_rate * duration) / size as f32
    }

    pub fn enqueue(self: &mut Self, value: T) {
        let value = value.abs();
        self.t -= 1.0;
        if self.t < 0.0 {
            self.buffer.enqueue(self.max_acc);
            self.t += self.sample_delta;
            self.max_acc = T::default();
        }
        if value > self.max_acc {
            self.max_acc = value
        }
    }

    pub fn len(&self) -> usize {
        self.buffer.len()
    }
}

impl PeakBuffer<f32> {
    /// Enqueues an entire [`Buffer`], mono-summing it if no channel is specified.
    pub fn enqueue_buffer(self: &mut Self, buffer: &mut Buffer, channel: Option<usize>) {
        match channel {
            Some(channel) => {
                for sample in buffer.as_slice()[channel].into_iter() {
                    self.enqueue(*sample);
                }
            }
            None => {
                for sample in buffer.iter_samples() {
                    self.enqueue(
                        (1. / (&sample).len() as f32) * sample.into_iter().map(|x| *x).sum::<f32>(),
                    );
                }
            }
        }
    }
}

impl<'a, T: Copy> IntoIterator for &'a PeakBuffer<T> {
    type Item = &'a T;
    type IntoIter = Iter<'a, T>;

    /// Creates an iterator from a reference.
    fn into_iter(self) -> Self::IntoIter {
        (&self.buffer).into_iter()
    }
}

impl<T: Debug + Copy> Debug for PeakBuffer<T> {
    fn fmt(&self, f: &mut Formatter) -> Result<(), std::fmt::Error> {
        self.buffer.fmt(f)
    }
}

impl<T> Empty for PeakBuffer<T> {
    fn is_empty(self: &Self) -> bool {
        self.buffer.is_empty()
    }
}
impl<T> Len for PeakBuffer<T> {
    /// Returns the length of the buffer.
    fn len(self: &Self) -> usize {
        self.buffer.len()
    }
}

impl<T> Index<usize> for PeakBuffer<T> {
    type Output = T;

    fn index(&self, index: usize) -> &Self::Output {
        self.buffer.index(index)
    }
}
impl<T> IndexMut<usize> for PeakBuffer<T> {
    fn index_mut(&mut self, index: usize) -> &mut Self::Output {
        self.buffer.index_mut(index)
    }
}

impl<T: Default> Deref for PeakBuffer<T> {
    type Target = [T];
    /// Dereferences the underlying data, giving you direct access to it.
    ///
    /// Crucially, this does not preserve the ordering you would get by
    /// iterating over the `RingBuffer` or indexing it directly.
    fn deref(&self) -> &Self::Target {
        self.buffer.deref()
    }
}
impl<T: Default> DerefMut for PeakBuffer<T> {
    /// Mutably dereferences the underlying data, giving you direct access to
    /// it.
    ///
    /// Crucially, this does not preserve the ordering you would get by
    /// iterating over the `RingBuffer` or indexing it directly.
    fn deref_mut(&mut self) -> &mut Self::Target {
        self.buffer.deref_mut()
    }
}

#[cfg(test)]
mod tests {
    use super::PeakBuffer;

    #[test]
    fn enqueue() {
        let mut rb = PeakBuffer::<f32>::new(16, 4.0, 8.0);

        rb.enqueue(2.);
        rb.enqueue(9.);
        rb.enqueue(19.);
        rb.enqueue(-10.);
        rb.enqueue(4.);
        rb.enqueue(6.);

        let buffer = rb.buffer.to_vec();

        assert_eq!(buffer[0], 9.);
        assert_eq!(buffer[1], 19.);
    }
}
