//! PC-side safety rules.
//!
//! The device enforces its own limits (arming, watchdog, ceiling, rise rate,
//! latched emergency stop) and would keep the bench safe on its own. This
//! module is the second, independent layer: it decides what the application
//! is willing to *ask* for, and when it must stop asking.
//!
//! It is pure logic with injected time, so every rule is unit tested.

use std::time::{Duration, Instant};

use serde::Serialize;

use crate::config::SafetyLimits;
use crate::domain::ActuatorState;
use crate::error::{Error, Result};
use crate::protocol::ids::sensor;

/// Facts gathered by the caller before an arming attempt.
#[derive(Debug, Clone)]
pub struct ArmPreconditions {
    pub connected: bool,
    /// The selected driver is able to command the ESC (not "manual").
    pub driver_can_command: bool,
    pub actuator: Option<ActuatorState>,
    /// The operator ticked the "area is clear" confirmation for this arming.
    pub operator_confirmed: bool,
}

/// Why the supervisor cut the throttle on its own.
#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(tag = "cause", rename_all = "snake_case")]
pub enum Trip {
    ThrustLimit { value: f64, limit: f64 },
    TorqueLimit { value: f64, limit: f64 },
    /// The window stopped reporting while armed (frozen or closed UI).
    UiSilent,
}

pub struct SafetySupervisor {
    limits: SafetyLimits,
    last_ui: Option<Instant>,
    last_setpoint: f64,
    last_setpoint_at: Option<Instant>,
}

impl SafetySupervisor {
    pub fn new(limits: SafetyLimits) -> Self {
        Self { limits, last_ui: None, last_setpoint: 0.0, last_setpoint_at: None }
    }

    pub fn set_limits(&mut self, limits: SafetyLimits) {
        self.limits = limits;
    }

    pub fn limits(&self) -> &SafetyLimits {
        &self.limits
    }

    /// Every condition must hold; the first one that does not is reported.
    pub fn check_arm(&self, pre: &ArmPreconditions) -> Result<()> {
        if !pre.connected {
            return Err(Error::Safety("no device is connected".into()));
        }
        if !pre.driver_can_command {
            return Err(Error::Safety("the selected throttle mode cannot command the ESC".into()));
        }
        match pre.actuator {
            Some(ActuatorState::Disarmed) => {}
            Some(ActuatorState::EStop) => {
                return Err(Error::Safety("the emergency stop is latched; clear it first".into()))
            }
            Some(ActuatorState::Failsafe) => {
                return Err(Error::Safety("the device is in failsafe; disarm it first".into()))
            }
            Some(ActuatorState::Armed) => return Err(Error::Safety("already armed".into())),
            None => return Err(Error::Safety("the device has not reported its state yet".into())),
        }
        if !pre.operator_confirmed {
            return Err(Error::Safety("confirm that the propeller area is clear".into()));
        }
        Ok(())
    }

    /// Call when arming succeeded: setpoints start from zero again.
    pub fn on_armed(&mut self, now: Instant) {
        self.last_setpoint = 0.0;
        self.last_setpoint_at = Some(now);
        self.last_ui = Some(now);
    }

    /// Call when the throttle was forced to zero for any reason.
    pub fn on_zeroed(&mut self) {
        self.last_setpoint = 0.0;
        self.last_setpoint_at = None;
    }

    /// Applies the ceiling and the rise-rate limit to a requested throttle.
    /// Reductions pass through unchanged: slowing down is never delayed.
    pub fn shape_setpoint(&mut self, requested: f64, now: Instant) -> f64 {
        let ceiling = self.limits.max_throttle.clamp(0.0, 1.0);
        let requested = if requested.is_finite() { requested.clamp(0.0, ceiling) } else { 0.0 };
        let shaped = if requested <= self.last_setpoint {
            requested
        } else {
            let dt = self.last_setpoint_at.map_or(0.0, |t| now.saturating_duration_since(t).as_secs_f64());
            requested.min(self.last_setpoint + self.limits.max_rise_per_s * dt)
        };
        self.last_setpoint = shaped;
        self.last_setpoint_at = Some(now);
        shaped
    }

    pub fn note_ui_alive(&mut self, now: Instant) {
        self.last_ui = Some(now);
    }

    /// True when the UI has been silent for longer than allowed.
    pub fn ui_expired(&self, now: Instant) -> bool {
        let timeout = Duration::from_millis(self.limits.ui_timeout_ms);
        self.last_ui.map_or(true, |t| now.saturating_duration_since(t) > timeout)
    }

    /// Checks one measurement against the configured limits.
    pub fn check_sample(&self, sensor_type: u8, value: f64) -> Option<Trip> {
        match sensor_type {
            sensor::LOAD_CELL_THRUST => self
                .limits
                .max_thrust_n
                .filter(|limit| value.abs() > *limit)
                .map(|limit| Trip::ThrustLimit { value, limit }),
            sensor::LOAD_CELL_TORQUE => self
                .limits
                .max_torque_nm
                .filter(|limit| value.abs() > *limit)
                .map(|limit| Trip::TorqueLimit { value, limit }),
            _ => None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn limits() -> SafetyLimits {
        SafetyLimits { max_throttle: 0.6, max_rise_per_s: 0.5, max_thrust_n: Some(40.0), max_torque_nm: None, ui_timeout_ms: 1500 }
    }

    fn ready() -> ArmPreconditions {
        ArmPreconditions {
            connected: true,
            driver_can_command: true,
            actuator: Some(ActuatorState::Disarmed),
            operator_confirmed: true,
        }
    }

    #[test]
    fn arming_requires_every_precondition() {
        let s = SafetySupervisor::new(limits());
        assert!(s.check_arm(&ready()).is_ok());
        assert!(s.check_arm(&ArmPreconditions { connected: false, ..ready() }).is_err());
        assert!(s.check_arm(&ArmPreconditions { driver_can_command: false, ..ready() }).is_err());
        assert!(s.check_arm(&ArmPreconditions { operator_confirmed: false, ..ready() }).is_err());
        assert!(s.check_arm(&ArmPreconditions { actuator: None, ..ready() }).is_err());
        for state in [ActuatorState::EStop, ActuatorState::Failsafe, ActuatorState::Armed] {
            assert!(s.check_arm(&ArmPreconditions { actuator: Some(state), ..ready() }).is_err());
        }
    }

    #[test]
    fn setpoint_is_capped_and_rise_limited_but_falls_immediately() {
        let mut s = SafetySupervisor::new(limits());
        let t0 = Instant::now();
        s.on_armed(t0);
        // 0.5 per second: after 100 ms at most 0.05.
        let a = s.shape_setpoint(1.0, t0 + Duration::from_millis(100));
        assert!((a - 0.05).abs() < 1e-9);
        // Long after, the ceiling applies.
        let b = s.shape_setpoint(1.0, t0 + Duration::from_secs(10));
        assert!((b - 0.6).abs() < 1e-9);
        // A reduction is not rate limited.
        assert_eq!(s.shape_setpoint(0.1, t0 + Duration::from_secs(10)), 0.1);
        assert_eq!(s.shape_setpoint(f64::NAN, t0 + Duration::from_secs(11)), 0.0);
    }

    #[test]
    fn ui_silence_expires() {
        let mut s = SafetySupervisor::new(limits());
        let t0 = Instant::now();
        assert!(s.ui_expired(t0)); // never heard from the UI
        s.note_ui_alive(t0);
        assert!(!s.ui_expired(t0 + Duration::from_millis(1400)));
        assert!(s.ui_expired(t0 + Duration::from_millis(1600)));
    }

    #[test]
    fn measurement_limits_trip() {
        let s = SafetySupervisor::new(limits());
        assert!(s.check_sample(sensor::LOAD_CELL_THRUST, 39.0).is_none());
        assert!(matches!(s.check_sample(sensor::LOAD_CELL_THRUST, -41.0), Some(Trip::ThrustLimit { .. })));
        assert!(s.check_sample(sensor::LOAD_CELL_TORQUE, 99.0).is_none()); // no torque limit set
    }
}
