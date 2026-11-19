//! Multi-point load-cell calibration.
//!
//! The operator applies known masses; for each one the application averages
//! the raw reading. A least-squares line through (reference, raw) gives the
//! channel's offset and scale, and the residuals show how well the cell and
//! the fixture behave.
//!
//! Every rig-specific distance is a parameter (`CalibrationOptions`), because
//! the mechanical design is expected to change.

use serde::{Deserialize, Serialize};

use crate::config::settings::CalibrationOptions;
use crate::error::{Error, Result};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ReferenceKind {
    /// Reference in newtons: mass x gravity x force ratio.
    Thrust,
    /// Reference in newton metres: mass x gravity x calibration arm.
    Torque,
}

/// Physical quantity produced by hanging `mass_g` on the calibration fixture.
pub fn reference_value(kind: ReferenceKind, mass_g: f64, gravity: f64, options: &CalibrationOptions) -> f64 {
    let weight_n = mass_g * 1e-3 * gravity;
    match kind {
        ReferenceKind::Thrust => weight_n * options.force_ratio,
        ReferenceKind::Torque => weight_n * options.torque_arm_mm * 1e-3,
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct CalPoint {
    pub mass_g: f64,
    /// Reference value in the channel's unit (N or N·m).
    pub reference: f64,
    pub raw_mean: f64,
    /// Standard deviation of the raw reading while capturing: noise indicator.
    pub raw_std: f64,
    pub samples: usize,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct CalFit {
    /// Raw counts per unit (always positive).
    pub scale: f64,
    /// Raw counts at zero load.
    pub offset: f64,
    /// True when the reading decreases with load (cell mounted reversed).
    pub invert: bool,
    pub r_squared: f64,
    /// Fit error at each point, in the channel's unit.
    pub residuals: Vec<f64>,
    pub max_error: f64,
    /// Largest error as a percentage of the calibrated span.
    pub nonlinearity_pct: f64,
}

/// Fits `raw = offset + slope * reference` by ordinary least squares.
pub fn fit(points: &[CalPoint]) -> Result<CalFit> {
    if points.len() < 2 {
        return Err(Error::Invalid("at least two calibration points are needed".into()));
    }
    let n = points.len() as f64;
    let mean_x = points.iter().map(|p| p.reference).sum::<f64>() / n;
    let mean_y = points.iter().map(|p| p.raw_mean).sum::<f64>() / n;
    let sxx: f64 = points.iter().map(|p| (p.reference - mean_x).powi(2)).sum();
    let sxy: f64 = points.iter().map(|p| (p.reference - mean_x) * (p.raw_mean - mean_y)).sum();
    let syy: f64 = points.iter().map(|p| (p.raw_mean - mean_y).powi(2)).sum();
    let span = points.iter().map(|p| p.reference).fold(f64::NEG_INFINITY, f64::max)
        - points.iter().map(|p| p.reference).fold(f64::INFINITY, f64::min);
    if sxx <= 0.0 || span <= 0.0 {
        return Err(Error::Invalid("the calibration points must use at least two different loads".into()));
    }
    let slope = sxy / sxx;
    if slope.abs() < 1e-9 {
        return Err(Error::Invalid("the reading does not change with load; check the wiring and the cell".into()));
    }
    let offset = mean_y - slope * mean_x;
    // Residuals expressed in engineering units.
    let residuals: Vec<f64> = points.iter().map(|p| (p.raw_mean - offset) / slope - p.reference).collect();
    let max_error = residuals.iter().fold(0.0_f64, |m, r| m.max(r.abs()));
    let r_squared = if syy > 0.0 { (sxy * sxy) / (sxx * syy) } else { 1.0 };
    Ok(CalFit {
        scale: slope.abs(),
        offset,
        invert: slope < 0.0,
        r_squared,
        residuals,
        max_error,
        nonlinearity_pct: max_error / span * 100.0,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn point(reference: f64, raw_mean: f64) -> CalPoint {
        CalPoint { mass_g: 0.0, reference, raw_mean, raw_std: 0.0, samples: 100 }
    }

    fn options() -> CalibrationOptions {
        crate::config::defaults().settings.calibration.clone()
    }

    #[test]
    fn reference_uses_local_gravity_and_geometry() {
        let mut o = options();
        o.force_ratio = 1.0;
        o.torque_arm_mm = 100.0;
        assert!((reference_value(ReferenceKind::Thrust, 1000.0, 9.78, &o) - 9.78).abs() < 1e-12);
        assert!((reference_value(ReferenceKind::Torque, 500.0, 9.78, &o) - 0.489).abs() < 1e-12);
    }

    #[test]
    fn recovers_offset_and_scale_exactly() {
        let points: Vec<_> = [0.0, 5.0, 10.0, 20.0].iter().map(|&x| point(x, 84_000.0 + 21_900.0 * x)).collect();
        let fit = fit(&points).unwrap();
        assert!((fit.scale - 21_900.0).abs() < 1e-6);
        assert!((fit.offset - 84_000.0).abs() < 1e-6);
        assert!(!fit.invert);
        assert!(fit.max_error < 1e-9);
        assert!((fit.r_squared - 1.0).abs() < 1e-12);
    }

    #[test]
    fn reversed_cell_is_reported_as_inverted() {
        let fit = fit(&[point(0.0, 1000.0), point(10.0, -9000.0)]).unwrap();
        assert!(fit.invert);
        assert!((fit.scale - 1000.0).abs() < 1e-9);
    }

    #[test]
    fn nonlinearity_is_relative_to_span() {
        // Middle point reads 0.1 N high over a 10 N span: about 0.67 % after the fit.
        let fit = fit(&[point(0.0, 0.0), point(5.0, 5100.0), point(10.0, 10_000.0)]).unwrap();
        assert!(fit.nonlinearity_pct > 0.5 && fit.nonlinearity_pct < 1.0, "{}", fit.nonlinearity_pct);
    }

    #[test]
    fn rejects_degenerate_input() {
        assert!(fit(&[point(1.0, 5.0)]).is_err());
        assert!(fit(&[point(1.0, 5.0), point(1.0, 6.0)]).is_err());
        assert!(fit(&[point(0.0, 5.0), point(1.0, 5.0)]).is_err());
    }
}
