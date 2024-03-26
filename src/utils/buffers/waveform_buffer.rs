use crate::utils::ring_buffer::{Iter, RingBuffer};

use len_trait::len::{Empty, Len};
use nih_plug::buffer::Buffer;
use num_traits::real::Real;
use std::{
    fmt::{Debug, Formatter},
    ops::{Deref, DerefMut, Index, IndexMut},
};

/// A special type of ring buffer, intended for use in peak waveform analysis.
///
/// This is a wrapper around the [`RingBuffer`](crate::utils::RingBuffer) struct
/// that specifically handles waveforms. It stores elements of type T in pairs
/// to represent the minimum and maximum values of a waveform over a certain
/// interval. It provides methods for setting the sample rate and duration, as
/// well as enqueueing new values and retrieving the stored waveform data.
///
/// For each pair `(T,T)` of samples that a WaveformBuffer holds, the first
/// element is the local minimum, and the second is the local maximum within the
/// respective time frame.
///
/// ![Alt version](http://127.0.0.1:5500/img.svg)
///
/// These values can be used to construct a zoomed-out representation of the
/// audio data without losing peak information - which is why this buffer is
/// used in the [`Oscilloscope`](crate::editor::views::Oscilloscope).
///
/// # Example
///
/// Here's how to create a `WaveformBuffer` with 512 samples, stored as f32
/// values. We'll provide a sample rate of 44.1 kHz and a length of 10 seconds.
///
/// ```
/// use cyma::utils::WaveformBuffer;
/// let mut rb = WaveformBuffer::<f32>::new(512, 10.0, 44100.);
/// ```
///
/// When we later push into this buffer, it will accumulate samples according to
/// these restrictions. It will take (44100*10)/512 enqueued samples for a new
/// pair of maximum and minimum values to be added to the buffer.
#[derive(Clone, PartialEq, Default)]
pub struct WaveformBuffer<T> {
    buffer: RingBuffer<(T, T)>,
    // Minimum and maximum accumulators
    min_acc: T,
    max_acc: T,
    // The gap between elements of the buffer in samples
    sample_delta: f32,
    // Used to calculate the sample_delta
    sample_rate: f32,
    duration: f32,
    // The current time, counts down from sample_delta to 0
    t: f32,
}

impl<T> WaveformBuffer<T>
where
    T: Clone + Copy + Default + Debug + PartialOrd + Real,
{
    /// Creates a new `WaveformBuffer` with the specified sample rate and
    /// duration (in seconds).
    pub fn new(size: usize, sample_rate: f32, duration: f32) -> Self {
        let sample_delta = Self::sample_delta(size, sample_rate as f32, duration as f32);
        Self {
            buffer: RingBuffer::<(T, T)>::new(size),
            min_acc: T::max_value(),
            max_acc: T::min_value(),
            sample_delta,
            sample_rate,
            duration,
            t: sample_delta,
        }
    }

    // TODO: Allow resizing without clearing
    /// Sets the size of the buffer and **clears** it.
    pub fn resize(self: &mut Self, size: usize) {
        if self.buffer.len() == size {
            return;
        };
        self.buffer.resize(size);
        self.sample_delta = Self::sample_delta(size, self.sample_rate, self.duration);
        self.buffer.clear();
    }

    /// Sets the sample rate of the buffer and **clears** it.
    pub fn set_sample_rate(self: &mut Self, sample_rate: f32) {
        self.sample_rate = sample_rate;
        self.sample_delta = Self::sample_delta(self.buffer.len(), sample_rate, self.duration);
        self.buffer.clear();
    }

    /// Sets the duration of the buffer (in seconds) and **clears** it.
    pub fn set_duration(self: &mut Self, duration: f32) {
        self.duration = duration;
        self.sample_delta = Self::sample_delta(self.buffer.len(), self.sample_rate, duration);
        self.buffer.clear();
    }

    fn sample_delta(size: usize, sample_rate: f32, duration: f32) -> f32 {
        (sample_rate * duration) / size as f32
    }

    /// Adds a new element of type `T` to the buffer.
    ///
    /// If the buffer is full, the oldest element is removed.
    pub fn enqueue(self: &mut Self, value: T) {
        self.t -= 1.0;
        if self.t < 0.0 {
            self.buffer.enqueue((self.min_acc, self.max_acc));
            self.t += self.sample_delta;
            self.min_acc = T::max_value();
            self.max_acc = T::min_value();
        }
        if value > self.max_acc {
            self.max_acc = value
        }
        if value < self.min_acc {
            self.min_acc = value
        }
    }

    /// Returns the length of the buffer.
    pub fn len(&self) -> usize {
        self.buffer.len()
    }
}

impl WaveformBuffer<f32> {
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

impl<'a, T: Copy> IntoIterator for &'a WaveformBuffer<T> {
    type Item = &'a (T, T);
    type IntoIter = Iter<'a, (T, T)>;

    /// Creates an iterator from a reference.
    fn into_iter(self) -> Self::IntoIter {
        (&self.buffer).into_iter()
    }
}

impl<T: Debug + Copy> Debug for WaveformBuffer<T> {
    fn fmt(&self, f: &mut Formatter) -> Result<(), std::fmt::Error> {
        self.buffer.fmt(f)
    }
}

impl<T> Index<usize> for WaveformBuffer<T> {
    type Output = (T, T);

    fn index(&self, index: usize) -> &Self::Output {
        self.buffer.index(index)
    }
}
impl<T> IndexMut<usize> for WaveformBuffer<T> {
    fn index_mut(&mut self, index: usize) -> &mut Self::Output {
        self.buffer.index_mut(index)
    }
}

impl<T> Empty for WaveformBuffer<T> {
    fn is_empty(self: &Self) -> bool {
        self.buffer.is_empty()
    }
}
impl<T> Len for WaveformBuffer<T> {
    /// Returns the length of the buffer.
    fn len(self: &Self) -> usize {
        self.buffer.len()
    }
}

impl<T: Default> Deref for WaveformBuffer<T> {
    type Target = [(T, T)];
    /// Dereferences the underlying data, giving you direct access to it.
    ///
    /// Crucially, this does not preserve the ordering you would get by
    /// iterating over the `RingBuffer` or indexing it directly.
    fn deref(&self) -> &Self::Target {
        self.buffer.deref()
    }
}
impl<T: Default> DerefMut for WaveformBuffer<T> {
    /// Mutably dereferences the underlying data, giving you direct access to
    /// it.
    ///
    /// Crucially, this does not preserve the ordering you would get by
    /// iterating over the `RingBuffer` or indexing it directly.
    fn deref_mut(&mut self) -> &mut Self::Target {
        self.buffer.deref_mut()
    }
}
