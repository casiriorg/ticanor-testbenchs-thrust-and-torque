//! Scripted throttle profiles (open loop).
//!
//! A profile is a JSON list of steps. `hold` keeps a throttle for a time;
//! `ramp` moves linearly from the previous throttle to the target. Running
//! the same profile for every motor/propeller pair is what makes recordings
//! comparable.

use serde::{Deserialize, Serialize};

use crate::error::{Error, Result};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum StepKind {
    Hold,
    Ramp,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Step {
    pub kind: StepKind,
    /// Throttle fraction, 0..1.
    pub target: f64,
    pub duration_s: f64,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Profile {
    pub name: String,
    #[serde(default)]
    pub description: String,
    pub steps: Vec<Step>,
}

impl Profile {
    pub fn validate(&self, max_throttle: f64) -> Result<()> {
        if self.steps.is_empty() {
            return Err(Error::Invalid("the profile has no steps".into()));
        }
        for (i, step) in self.steps.iter().enumerate() {
            if !(0.0..=1.0).contains(&step.target) {
                return Err(Error::Invalid(format!("step {}: target must be between 0 and 1", i + 1)));
            }
            if step.target > max_throttle + 1e-9 {
                return Err(Error::Safety(format!(
                    "step {} asks for {:.0} % but the throttle limit is {:.0} %",
                    i + 1,
                    step.target * 100.0,
                    max_throttle * 100.0
                )));
            }
            if !(step.duration_s > 0.0 && step.duration_s <= 3600.0) {
                return Err(Error::Invalid(format!("step {}: duration must be between 0 and 3600 s", i + 1)));
            }
        }
        Ok(())
    }

    pub fn total_duration(&self) -> f64 {
        self.steps.iter().map(|s| s.duration_s).sum()
    }

    /// Throttle at `elapsed` seconds, or `None` once the profile has finished.
    pub fn setpoint_at(&self, elapsed: f64) -> Option<f64> {
        let mut start = 0.0;
        let mut previous = 0.0;
        for step in &self.steps {
            let end = start + step.duration_s;
            if elapsed < end {
                return Some(match step.kind {
                    StepKind::Hold => step.target,
                    StepKind::Ramp => previous + (step.target - previous) * (elapsed - start) / step.duration_s,
                });
            }
            start = end;
            previous = step.target;
        }
        None
    }

    /// Ready-made profiles offered by the UI. All of them end at zero.
    pub fn builtins(max_throttle: f64) -> Vec<Profile> {
        let top = (max_throttle * 10.0).floor() / 10.0;
        let mut staircase = Vec::new();
        let mut level = 0.1;
        while level <= top + 1e-9 {
            staircase.push(Step { kind: StepKind::Hold, target: (level * 10.0_f64).round() / 10.0, duration_s: 4.0 });
            level += 0.1;
        }
        staircase.push(Step { kind: StepKind::Hold, target: 0.0, duration_s: 1.0 });
        vec![
            Profile {
                name: "Staircase 10 % steps".into(),
                description: "Holds each 10 % level for 4 s up to the throttle limit. Best for thrust-versus-throttle curves.".into(),
                steps: staircase,
            },
            Profile {
                name: "Slow ramp".into(),
                description: "Ramps to the throttle limit in 20 s and back in 10 s. Shows hysteresis and resonances.".into(),
                steps: vec![
                    Step { kind: StepKind::Ramp, target: top, duration_s: 20.0 },
                    Step { kind: StepKind::Hold, target: top, duration_s: 2.0 },
                    Step { kind: StepKind::Ramp, target: 0.0, duration_s: 10.0 },
                ],
            },
            Profile {
                name: "Step response".into(),
                description: "Jumps between 20 % and 50 % to measure how fast the motor and propeller respond.".into(),
                steps: vec![
                    Step { kind: StepKind::Hold, target: 0.2_f64.min(top), duration_s: 3.0 },
                    Step { kind: StepKind::Hold, target: 0.5_f64.min(top), duration_s: 3.0 },
                    Step { kind: StepKind::Hold, target: 0.2_f64.min(top), duration_s: 3.0 },
                    Step { kind: StepKind::Hold, target: 0.0, duration_s: 1.0 },
                ],
            },
        ]
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn interpolates_ramps_and_ends() {
        let profile = Profile {
            name: "t".into(),
            description: String::new(),
            steps: vec![
                Step { kind: StepKind::Ramp, target: 0.4, duration_s: 2.0 },
                Step { kind: StepKind::Hold, target: 0.4, duration_s: 1.0 },
                Step { kind: StepKind::Ramp, target: 0.0, duration_s: 1.0 },
            ],
        };
        assert_eq!(profile.setpoint_at(0.0), Some(0.0));
        assert!((profile.setpoint_at(1.0).unwrap() - 0.2).abs() < 1e-12);
        assert_eq!(profile.setpoint_at(2.5), Some(0.4));
        assert!((profile.setpoint_at(3.5).unwrap() - 0.2).abs() < 1e-12);
        assert_eq!(profile.setpoint_at(4.0), None);
        assert_eq!(profile.total_duration(), 4.0);
    }

    #[test]
    fn builtins_respect_the_limit_and_validate() {
        for profile in Profile::builtins(0.6) {
            profile.validate(0.6).unwrap();
            assert_eq!(profile.steps.last().unwrap().target, 0.0);
        }
        let over = Profile { name: "x".into(), description: String::new(), steps: vec![Step { kind: StepKind::Hold, target: 0.8, duration_s: 1.0 }] };
        assert!(matches!(over.validate(0.6), Err(Error::Safety(_))));
    }
}
