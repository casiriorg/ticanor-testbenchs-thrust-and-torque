use serde::Serialize;

use super::Series;

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct Stats {
    pub count: usize,
    pub mean: f64,
    /// Sample standard deviation.
    pub std: f64,
    pub min: f64,
    pub max: f64,
    pub peak_to_peak: f64,
    /// Mean sample rate over the window, in Hz.
    pub rate_hz: f64,
}

pub fn mean(values: &[f64]) -> f64 {
    if values.is_empty() {
        return f64::NAN;
    }
    values.iter().sum::<f64>() / values.len() as f64
}

pub fn std_dev(values: &[f64]) -> f64 {
    if values.len() < 2 {
        return 0.0;
    }
    let m = mean(values);
    (values.iter().map(|v| (v - m).powi(2)).sum::<f64>() / (values.len() - 1) as f64).sqrt()
}

pub fn describe(series: &Series) -> Option<Stats> {
    if series.is_empty() {
        return None;
    }
    let min = series.v.iter().copied().fold(f64::INFINITY, f64::min);
    let max = series.v.iter().copied().fold(f64::NEG_INFINITY, f64::max);
    let span = series.t[series.len() - 1] - series.t[0];
    Some(Stats {
        count: series.len(),
        mean: mean(&series.v),
        std: std_dev(&series.v),
        min,
        max,
        peak_to_peak: max - min,
        rate_hz: if span > 0.0 { (series.len() - 1) as f64 / span } else { 0.0 },
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn describes_a_window() {
        let series = Series { t: vec![0.0, 0.5, 1.0, 1.5, 2.0], v: vec![1.0, 2.0, 3.0, 4.0, 5.0] };
        let stats = describe(&series).unwrap();
        assert_eq!(stats.count, 5);
        assert_eq!(stats.mean, 3.0);
        assert!((stats.std - 1.5811388).abs() < 1e-6);
        assert_eq!(stats.peak_to_peak, 4.0);
        assert_eq!(stats.rate_hz, 2.0);
        assert!(describe(&Series::default()).is_none());
        assert_eq!(describe(&series.window(0.4, 1.1)).unwrap().count, 2);
    }
}
