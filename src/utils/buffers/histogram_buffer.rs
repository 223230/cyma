use super::VisualizerBuffer;
use std::fmt::Debug;
use std::ops::{Index, IndexMut};

/// This buffer creates histogram data with variable decay from a signal.
///
/// After an element is added, all elements are scaled so the largest element has value 1
/// Due to its fixed-size nature, the histogram buffer is very fast and doesn't dynamically reallocate itself.
// #[derive(Clone, PartialEq, Eq, Default, Hash, Debug)]
#[derive(Clone, PartialEq, Default, Debug)]
pub struct HistogramBuffer {
    size: usize,
    data: Vec<f32>,
    sample_rate: f32,
    // The decay time.
    decay: f32,
    // when a sample is added to a bin, add this number to that bin
    // then scale the whole vector so the max is 1
    // together these make older values decay; the smaller decay_weight, the faster the decay
    decay_weight: f32,
    edges: Vec<f32>,
    range: (f32, f32),
}

impl HistogramBuffer {
    /// Constructs a new HistogramBuffer with the given size.
    ///
    /// * `size` - The number of bins; Usually, this can be kept < 2000
    /// * `decay` - The rate of decay
    ///
    /// The buffer needs to be provided a sample rate after initialization - do this by
    /// calling [`set_sample_rate`](Self::set_sample_rate) inside your
    /// [`initialize()`](nih_plug::plugin::Plugin::initialize) function.
    pub fn new(size: usize, decay: f32) -> Self {
        let decay_weight = Self::decay_weight(decay, 48000.);
        Self {
            size,
            data: vec![f32::default(); size],
            sample_rate: 48000.,
            decay,
            decay_weight,
            edges: vec![f32::default(); size - 1],
            range: (-96., 24.),
        }
    }

    pub(crate) fn set_range(&mut self, range: (f32, f32)) {
        self.range = range;
        self.update();
    }

    /// Sets the decay time of the `HistogramBuffer`.
    ///
    /// * `decay` - The time it takes for a sample inside the buffer to decrease by -12dB, in milliseconds
    pub fn set_decay(self: &mut Self, decay: f32) {
        self.decay = decay;
        self.update();
    }

    /// Sets the sample rate of the incoming audio.
    ///
    /// This function **clears** the buffer. You can call it inside your
    /// [`initialize()`](nih_plug::plugin::Plugin::initialize) function and provide the
    /// sample rate like so:
    ///
    /// ```
    /// fn initialize(
    ///     &mut self,
    ///     _audio_io_layout: &AudioIOLayout,
    ///     buffer_config: &BufferConfig,
    ///     _context: &mut impl InitContext<Self>,
    /// ) -> bool {
    ///     match self.histogram_buffer.lock() {
    ///         Ok(mut buffer) => {
    ///             buffer.set_sample_rate(buffer_config.sample_rate);
    ///         }
    ///         Err(_) => return false,
    ///     }
    ///
    ///     true
    /// }
    /// ```
    pub fn set_sample_rate(self: &mut Self, sample_rate: f32) {
        self.sample_rate = sample_rate;
        self.update();
        self.clear();
    }

    fn decay_weight(decay: f32, sample_rate: f32) -> f32 {
        0.25f64.powf((decay as f64 * sample_rate as f64).recip()) as f32
    }

    fn update(self: &mut Self) {
        // calculate the linear edge values from MIN_EDGE to MAX_EDGE, evenly spaced in the db domain
        let nr_edges: usize = self.size - 1;
        self.edges = (0..nr_edges)
            .map(|x| {
                Self::db_to_linear(
                    self.range.0
                        + x as f32 * ((self.range.1 - self.range.0) / (nr_edges as f32 - 1.0)),
                )
            })
            .collect::<Vec<_>>();

        self.decay_weight = Self::decay_weight(self.decay, self.sample_rate);
    }

    fn db_to_linear(db: f32) -> f32 {
        10.0_f32.powf(db / 20.0)
    }

    // Function to find the bin for a given linear audio value
    fn find_bin(&self, value: f32) -> usize {
        // Check if the value is smaller than the first edge
        if value < self.edges[0] {
            // if value < f32::EPSILON {
            // if value == 0.0 {
            return 0;
        }
        // Check if the value is larger than the last edge
        if value > *self.edges.last().unwrap() {
            return self.edges.len() as usize;
        }
        // Binary search to find the bin for the given value
        let mut left = 0;
        let mut right = self.edges.len() - 1;

        while left <= right {
            let mid = left + (right - left) / 2;
            if value >= self.edges[mid] {
                left = mid + 1;
            } else {
                right = mid - 1;
            }
        }
        // Return the bin index
        left as usize
    }
}

impl VisualizerBuffer<f32> for HistogramBuffer {
    /// Enqueues a single element.
    ///
    /// **Where possible, use [`enqueue_buffer`] instead!**
    fn enqueue(&mut self, value: f32) {
        let value = value.abs();
        // don't enqueue silence
        if value > 0.0 {
            let bin_index = self.find_bin(value);
            for i in 0..self.size - 1 {
                // decay all values
                self.data[i] *= self.decay_weight;
            }
            self.data[bin_index] += (1.0 - self.decay_weight); // Increment the count for the bin
        }
    }

    fn enqueue_buffer(&mut self, buffer: &mut nih_plug::buffer::Buffer, channel: Option<usize>) {
        // don't enqueue silence
        if !match channel {
            Some(channel) => buffer.as_slice()[channel]
                .iter()
                .any(|sample| *sample != 0.0),
            None => buffer
                .as_slice()
                .iter()
                .any(|channel| channel.iter().any(|sample| *sample != 0.0)),
        } {
            return;
        }

        // "Pre-decay" all values
        let decay_weight = self.decay_weight.powi(buffer.samples() as i32);

        for i in 0..self.size - 1 {
            self.data[i] *= decay_weight;
        }

        match channel {
            Some(channel) => {
                for sample in buffer.as_slice()[channel].iter() {
                    let value = (*sample).abs();
                    let bin_index = self.find_bin(value);
                    self.data[bin_index] += 1.0 - self.decay_weight; // Increment the count for the bin
                }
            }
            None => {
                for sample in buffer.iter_samples() {
                    let value = (1. / sample.len() as f32)
                        * sample.into_iter().map(|x| *x).sum::<f32>().abs();

                    let bin_index = self.find_bin(value);
                    self.data[bin_index] += 1.0 - self.decay_weight; // Increment the count for the bin
                }
            }
        };
    }

    /// Resizes the buffer to the given size, **clearing it**.
    fn resize(self: &mut Self, size: usize) {
        if size == self.len() {
            return;
        }
        self.clear();
        self.size = size;
        self.update();
    }

    /// Clears the entire buffer, filling it with default values (usually 0)
    fn clear(self: &mut Self) {
        self.data.iter_mut().for_each(|x| *x = f32::default());
    }

    fn len(self: &Self) -> usize {
        self.size
    }

    /// Grows the buffer, **clearing it**.
    fn grow(self: &mut Self, size: usize) {
        self.resize(size);
    }

    /// Shrinks the buffer, **clearing it**.
    fn shrink(self: &mut Self, size: usize) {
        self.resize(size)
    }
}

impl Index<usize> for HistogramBuffer {
    type Output = f32;

    fn index(&self, index: usize) -> &Self::Output {
        if index >= self.size {
            panic!(
                "Invalid histogram buffer access: Index {} is out of range for histogram buffer of size {}",
                index, self.size
            );
        }
        &self.data[index]
    }
}
impl IndexMut<usize> for HistogramBuffer {
    fn index_mut(&mut self, index: usize) -> &mut Self::Output {
        if index >= self.size {
            panic!(
                "Invalid histogram buffer access: Index {} is out of range for histogram buffer of size {}",
                index, self.size
                );
        }
        &mut self.data[index]
    }
}
