//! Spectral analysis and simple display filters.

use rustfft::{num_complex::Complex, FftPlanner};
use serde::Serialize;

use super::stats::mean;
use super::Series;
use crate::error::{Error, Result};

/// Median sample rate of a series, robust to occasional gaps.
pub fn estimate_rate(series: &Series) -> Option<f64> {
    if series.len() < 3 {
        return None;
    }
    let mut deltas: Vec<f64> = series.t.windows(2).map(|w| w[1] - w[0]).filter(|d| *d > 0.0).collect();
    if deltas.is_empty() {
        return None;
    }
    deltas.sort_by(|a, b| a.total_cmp(b));
    Some(1.0 / deltas[deltas.len() / 2])
}

/// Linear interpolation onto an even grid. The HX711 free-runs on its own
/// oscillator, so its samples are close to, but not exactly, evenly spaced.
pub fn resample_uniform(series: &Series, rate_hz: f64) -> Vec<f64> {
    if series.len() < 2 || rate_hz <= 0.0 {
        return series.v.clone();
    }
    let t0 = series.t[0];
    let count = ((series.t[series.len() - 1] - t0) * rate_hz).floor() as usize + 1;
    let mut out = Vec::with_capacity(count);
    let mut j = 0;
    for i in 0..count {
        let t = t0 + i as f64 / rate_hz;
        while j + 2 < series.len() && series.t[j + 1] < t {
            j += 1;
        }
        let (ta, tb) = (series.t[j], series.t[j + 1]);
        let w = if tb > ta { ((t - ta) / (tb - ta)).clamp(0.0, 1.0) } else { 0.0 };
        out.push(series.v[j] + (series.v[j + 1] - series.v[j]) * w);
    }
    out
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct Spectrum {
    pub freq_hz: Vec<f64>,
    /// Peak amplitude per bin, in the signal's unit (a sine of amplitude A
    /// centred on a bin reads A).
    pub amplitude: Vec<f64>,
    pub rate_hz: f64,
    pub resolution_hz: f64,
    /// Number of averaged segments; more segments mean a smoother estimate.
    pub segments: usize,
}

/// Welch estimate: Hann-windowed, 50 % overlapping, mean-removed segments.
/// `segment_len` is rounded down to a power of two; `None` picks one that
/// yields roughly eight segments.
pub fn amplitude_spectrum(series: &Series, segment_len: Option<usize>) -> Result<Spectrum> {
    let rate = estimate_rate(series).ok_or_else(|| Error::Invalid("not enough samples for a spectrum".into()))?;
    let data = resample_uniform(series, rate);
    if data.len() < 16 {
        return Err(Error::Invalid("not enough samples for a spectrum".into()));
    }
    let wanted = segment_len.unwrap_or(data.len() / 4).clamp(16, data.len());
    let n = 1usize << (usize::BITS - 1 - wanted.leading_zeros()); // previous power of two
    let hop = n / 2;
    let window: Vec<f64> =
        (0..n).map(|i| 0.5 - 0.5 * (2.0 * std::f64::consts::PI * i as f64 / n as f64).cos()).collect();
    let window_sum: f64 = window.iter().sum();

    let fft = FftPlanner::<f64>::new().plan_fft_forward(n);
    let bins = n / 2 + 1;
    let mut power = vec![0.0; bins];
    let mut segments = 0;
    let mut start = 0;
    while start + n <= data.len() {
        let segment = &data[start..start + n];
        let m = mean(segment);
        let mut buffer: Vec<Complex<f64>> =
            segment.iter().zip(&window).map(|(v, w)| Complex::new((v - m) * w, 0.0)).collect();
        fft.process(&mut buffer);
        for (k, slot) in power.iter_mut().enumerate() {
            // Single-sided amplitude; DC and Nyquist are not doubled.
            let factor = if k == 0 || (n % 2 == 0 && k == n / 2) { 1.0 } else { 2.0 };
            let amplitude = factor * buffer[k].norm() / window_sum;
            *slot += amplitude * amplitude;
        }
        segments += 1;
        start += hop;
    }
    let amplitude = power.iter().map(|p| (p / segments as f64).sqrt()).collect();
    let resolution = rate / n as f64;
    Ok(Spectrum {
        freq_hz: (0..bins).map(|k| k as f64 * resolution).collect(),
        amplitude,
        rate_hz: rate,
        resolution_hz: resolution,
        segments,
    })
}

/// Centred moving average of `n` samples (n <= 1 returns the input).
pub fn moving_average(values: &[f64], n: usize) -> Vec<f64> {
    if n <= 1 || values.is_empty() {
        return values.to_vec();
    }
    let half = n / 2;
    let mut prefix = Vec::with_capacity(values.len() + 1);
    prefix.push(0.0);
    for v in values {
        prefix.push(prefix[prefix.len() - 1] + v);
    }
    (0..values.len())
        .map(|i| {
            let a = i.saturating_sub(half);
            let b = (i + n - half).min(values.len());
            (prefix[b] - prefix[a]) / (b - a) as f64
        })
        .collect()
}

/// Where a mains-frequency disturbance shows up after sampling. At 80 SPS
/// the Nyquist limit is 40 Hz, so 50 Hz hum appears at 30 Hz and 60 Hz hum at
/// 20 Hz. (At 10 SPS the HX711's own filter already rejects both.)
pub fn alias_frequency(signal_hz: f64, rate_hz: f64) -> f64 {
    let folded = signal_hz % rate_hz;
    if folded > rate_hz / 2.0 {
        rate_hz - folded
    } else {
        folded
    }
}

/// Second-order notch (RBJ biquad), applied forward and backward so the
/// filtered trace is not shifted in time. Intended for display only: the
/// recorded data is never filtered.
pub fn notch(values: &[f64], rate_hz: f64, notch_hz: f64, q: f64) -> Vec<f64> {
    if values.len() < 3 || notch_hz <= 0.0 || notch_hz >= rate_hz / 2.0 {
        return values.to_vec();
    }
    let w0 = 2.0 * std::f64::consts::PI * notch_hz / rate_hz;
    let alpha = w0.sin() / (2.0 * q);
    let a0 = 1.0 + alpha;
    let (b0, b1, b2) = (1.0 / a0, -2.0 * w0.cos() / a0, 1.0 / a0);
    let (a1, a2) = (-2.0 * w0.cos() / a0, (1.0 - alpha) / a0);
    let pass = |input: &[f64]| -> Vec<f64> {
        // Start in steady state at the first value to avoid a start-up transient.
        let (mut x1, mut x2, mut y1, mut y2) = (input[0], input[0], input[0], input[0]);
        input
            .iter()
            .map(|&x| {
                let y = b0 * x + b1 * x1 + b2 * x2 - a1 * y1 - a2 * y2;
                x2 = x1;
                x1 = x;
                y2 = y1;
                y1 = y;
                y
            })
            .collect()
    };
    let mut forward = pass(values);
    forward.reverse();
    let mut both = pass(&forward);
    both.reverse();
    both
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sine(rate: f64, seconds: f64, freq: f64, amplitude: f64, offset: f64) -> Series {
        let n = (rate * seconds) as usize;
        let t: Vec<f64> = (0..n).map(|i| i as f64 / rate).collect();
        let v = t.iter().map(|t| offset + amplitude * (2.0 * std::f64::consts::PI * freq * t).sin()).collect();
        Series { t, v }
    }

    #[test]
    fn spectrum_finds_frequency_and_amplitude() {
        let series = sine(80.0, 16.0, 10.0, 0.5, 20.0);
        let spectrum = amplitude_spectrum(&series, Some(256)).unwrap();
        let (peak_index, peak) =
            spectrum.amplitude.iter().enumerate().fold((0, 0.0), |best, (i, &a)| if a > best.1 { (i, a) } else { best });
        assert!((spectrum.freq_hz[peak_index] - 10.0).abs() <= spectrum.resolution_hz);
        assert!((peak - 0.5).abs() < 0.03, "peak amplitude {peak}");
        assert!(spectrum.amplitude[0] < 1e-6, "the mean must be removed");
        assert!((spectrum.rate_hz - 80.0).abs() < 1e-6);
        assert!(spectrum.segments > 1);
    }

    #[test]
    fn resampling_fixes_uneven_timestamps() {
        let series = Series { t: vec![0.0, 0.09, 0.21, 0.3], v: vec![0.0, 0.9, 2.1, 3.0] };
        let even = resample_uniform(&series, 10.0);
        assert_eq!(even.len(), 4);
        for (i, v) in even.iter().enumerate() {
            assert!((v - i as f64).abs() < 1e-9);
        }
    }

    #[test]
    fn mains_hum_aliases_as_documented() {
        assert!((alias_frequency(50.0, 80.0) - 30.0).abs() < 1e-9);
        assert!((alias_frequency(60.0, 80.0) - 20.0).abs() < 1e-9);
    }

    #[test]
    fn notch_removes_its_frequency_and_keeps_the_mean() {
        let series = sine(80.0, 8.0, 30.0, 1.0, 5.0);
        let filtered = notch(&series.v, 80.0, 30.0, 5.0);
        let middle = &filtered[160..480];
        let ripple = middle.iter().fold(0.0_f64, |m, v| m.max((v - 5.0).abs()));
        assert!(ripple < 0.05, "residual ripple {ripple}");
    }

    #[test]
    fn moving_average_preserves_length_and_constant_signals() {
        assert_eq!(moving_average(&[2.0; 10], 4), vec![2.0; 10]);
        assert_eq!(moving_average(&[1.0, 2.0, 3.0], 1), vec![1.0, 2.0, 3.0]);
    }
}
