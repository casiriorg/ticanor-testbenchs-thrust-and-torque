//! Stateless analysis commands. The frontend sends the series it wants
//! analysed (a live buffer or a window of a recording), so the same commands
//! serve real-time and recorded data.

use tb_core::analysis::curves::{detect_plateaus, throttle_curve, CurvePoint};
use tb_core::analysis::spectrum::{amplitude_spectrum, Spectrum};
use tb_core::analysis::stats::{describe, Stats};
use tb_core::analysis::Series;

use super::CommandResult;

#[tauri::command(async)]
pub fn compute_stats(series: Series) -> Option<Stats> {
    describe(&series)
}

#[tauri::command(async)]
pub fn compute_spectrum(series: Series, segment_len: Option<usize>) -> CommandResult<Spectrum> {
    amplitude_spectrum(&series, segment_len)
}

/// Thrust (and torque) versus throttle from the plateaus of a recording.
#[tauri::command(async)]
pub fn compute_curve(throttle: Series, thrust: Series, torque: Option<Series>) -> Vec<CurvePoint> {
    let plateaus = detect_plateaus(&throttle, 0.01, 1.0);
    throttle_curve(&plateaus, &thrust, torque.as_ref(), 0.4)
}
