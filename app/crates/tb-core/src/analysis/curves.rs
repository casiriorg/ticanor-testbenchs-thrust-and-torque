//! Steady-state curves: what the motor and propeller deliver at each throttle.
//!
//! A recording made with a staircase profile contains plateaus of constant
//! throttle. Averaging the settled part of every plateau gives one point of
//! the thrust-versus-throttle (and torque-versus-throttle) curve, which is
//! the basic tool for comparing motors and propellers.

use serde::Serialize;

use super::stats::{mean, std_dev};
use super::Series;

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct Plateau {
    pub t0: f64,
    pub t1: f64,
    /// Throttle fraction, 0..1.
    pub throttle: f64,
}

/// Finds stretches where the throttle stays within `tolerance` of its first
/// value for at least `min_duration` seconds. Idle (zero) plateaus are skipped.
pub fn detect_plateaus(throttle: &Series, tolerance: f64, min_duration: f64) -> Vec<Plateau> {
    let mut plateaus = Vec::new();
    let mut start = 0;
    let n = throttle.len();
    for i in 1..=n {
        let broke = i == n || (throttle.v[i] - throttle.v[start]).abs() > tolerance;
        if !broke {
            continue;
        }
        let (t0, t1) = (throttle.t[start], throttle.t[i - 1]);
        let level = mean(&throttle.v[start..i]);
        if t1 - t0 >= min_duration && level > tolerance {
            plateaus.push(Plateau { t0, t1, throttle: level });
        }
        start = i;
    }
    plateaus
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct CurvePoint {
    pub throttle: f64,
    pub t0: f64,
    pub t1: f64,
    pub thrust_mean: f64,
    pub thrust_std: f64,
    pub torque_mean: Option<f64>,
    pub torque_std: Option<f64>,
    /// Thrust per unit torque (1/m). Higher means more thrust for the same
    /// shaft torque; useful to compare propellers on one motor.
    pub thrust_per_torque: Option<f64>,
    pub samples: usize,
}

/// Averages each plateau after discarding its first `settle_fraction`, where
/// the motor is still accelerating.
pub fn throttle_curve(
    plateaus: &[Plateau],
    thrust: &Series,
    torque: Option<&Series>,
    settle_fraction: f64,
) -> Vec<CurvePoint> {
    plateaus
        .iter()
        .filter_map(|p| {
            let t0 = p.t0 + (p.t1 - p.t0) * settle_fraction.clamp(0.0, 0.9);
            let thrust_window = thrust.window(t0, p.t1);
            if thrust_window.is_empty() {
                return None;
            }
            let thrust_mean = mean(&thrust_window.v);
            let torque_window = torque.map(|s| s.window(t0, p.t1)).filter(|w| !w.is_empty());
            let torque_mean = torque_window.as_ref().map(|w| mean(&w.v));
            Some(CurvePoint {
                throttle: p.throttle,
                t0,
                t1: p.t1,
                thrust_mean,
                thrust_std: std_dev(&thrust_window.v),
                torque_mean,
                torque_std: torque_window.as_ref().map(|w| std_dev(&w.v)),
                thrust_per_torque: torque_mean.filter(|q| q.abs() > 1e-6).map(|q| thrust_mean / q),
                samples: thrust_window.len(),
            })
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Staircase: 2 s at 0, 0.2, 0.4 and back to 0, sampled at 50 Hz.
    fn staircase() -> Series {
        let mut s = Series::default();
        for i in 0..400 {
            let t = i as f64 * 0.02;
            s.t.push(t);
            s.v.push(match i / 100 {
                1 => 0.2,
                2 => 0.4,
                _ => 0.0,
            });
        }
        s
    }

    #[test]
    fn finds_non_idle_plateaus() {
        let plateaus = detect_plateaus(&staircase(), 0.01, 1.0);
        assert_eq!(plateaus.len(), 2);
        assert!((plateaus[0].throttle - 0.2).abs() < 1e-9);
        assert!((plateaus[1].t0 - 4.0).abs() < 1e-9);
    }

    #[test]
    fn curve_uses_only_the_settled_part() {
        let throttle = staircase();
        // Thrust lags: it is still low for the first half second of a plateau.
        let thrust = Series {
            t: throttle.t.clone(),
            v: throttle
                .t
                .iter()
                .zip(&throttle.v)
                .map(|(t, u)| if (t % 2.0) < 0.5 { 0.0 } else { 100.0 * u * u })
                .collect(),
        };
        let torque = Series { t: thrust.t.clone(), v: thrust.v.iter().map(|f| f / 50.0).collect() };
        let curve = throttle_curve(&detect_plateaus(&throttle, 0.01, 1.0), &thrust, Some(&torque), 0.4);
        assert_eq!(curve.len(), 2);
        assert!((curve[0].thrust_mean - 4.0).abs() < 1e-9);
        assert!((curve[1].thrust_mean - 16.0).abs() < 1e-9);
        assert!((curve[1].thrust_per_torque.unwrap() - 50.0).abs() < 1e-9);
        assert!(curve[0].thrust_std < 1e-9);
    }
}
