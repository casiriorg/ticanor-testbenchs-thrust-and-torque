//! Signal analysis used live and on recordings.
//!
//! * `stats`    - descriptive statistics of a time window
//! * `spectrum` - resampling, Welch amplitude spectrum, display filters
//! * `curves`   - throttle plateaus and thrust/torque-versus-throttle curves
//!
//! Heavier or exploratory analysis belongs in the Python package under
//! `analysis/`, which reads the same recordings.

pub mod curves;
pub mod spectrum;
pub mod stats;

use serde::{Deserialize, Serialize};

/// A sampled signal. `t` is in seconds and increases (not necessarily evenly).
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct Series {
    pub t: Vec<f64>,
    pub v: Vec<f64>,
}

impl Series {
    pub fn len(&self) -> usize {
        self.t.len()
    }

    pub fn is_empty(&self) -> bool {
        self.t.is_empty()
    }

    /// Copy of the samples with `t0 <= t <= t1`.
    pub fn window(&self, t0: f64, t1: f64) -> Series {
        let start = self.t.partition_point(|&t| t < t0);
        let end = self.t.partition_point(|&t| t <= t1);
        Series { t: self.t[start..end].to_vec(), v: self.v[start..end].to_vec() }
    }
}
