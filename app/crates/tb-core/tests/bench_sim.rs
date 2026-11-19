//! End-to-end tests of the bench service against the in-process simulator:
//! connection, driver choice, safety interlocks, recording and analysis.

use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use tb_core::analysis::curves::{detect_plateaus, throttle_curve};
use tb_core::domain::{ActuatorState, DriverKind};
use tb_core::profile::{Profile, Step, StepKind};
use tb_core::recording::{load_recording, recording_dir, RecordingState, TestSetup};
use tb_core::services::bench::{Bench, BenchConfig, BenchEvent, BenchSnapshot, ConnectionState, EventSink};
use tb_core::Error;

#[derive(Default)]
struct Collector {
    alarms: Mutex<Vec<String>>,
    samples: Mutex<usize>,
}

impl EventSink for Collector {
    fn emit(&self, event: BenchEvent) {
        match event {
            BenchEvent::Alarm(alarm) => self.alarms.lock().unwrap().push(alarm.code),
            BenchEvent::Samples(batch) => *self.samples.lock().unwrap() += batch.len(),
            BenchEvent::Snapshot(_) => {}
        }
    }
}

struct Fixture {
    bench: Arc<Bench>,
    sink: Arc<Collector>,
    _dirs: tempfile::TempDir,
}

fn start() -> Fixture {
    let dirs = tempfile::tempdir().unwrap();
    // Auto-connect off: the test machine may have unrelated serial devices.
    std::fs::create_dir_all(dirs.path().join("config")).unwrap();
    std::fs::write(dirs.path().join("config/settings.json"), r#"{"auto_connect": false}"#).unwrap();
    let sink = Arc::new(Collector::default());
    let bench = Bench::start(
        BenchConfig {
            config_dir: dirs.path().join("config"),
            default_data_dir: dirs.path().join("data"),
            app_version: "test".into(),
        },
        sink.clone(),
    );
    bench.connect_simulator().unwrap();
    wait_for(&bench, |s| matches!(s.connection, ConnectionState::Connected { .. }) && s.status.is_some());
    Fixture { bench, sink, _dirs: dirs }
}

fn wait_for(bench: &Bench, condition: impl Fn(&BenchSnapshot) -> bool) -> BenchSnapshot {
    let deadline = Instant::now() + Duration::from_secs(8);
    loop {
        let snapshot = bench.snapshot();
        if condition(&snapshot) {
            return snapshot;
        }
        assert!(Instant::now() < deadline, "condition not reached; last snapshot: {snapshot:?}");
        std::thread::sleep(Duration::from_millis(20));
    }
}

/// Keeps the UI heartbeat going, as the window does.
fn keep_alive(bench: &Arc<Bench>, duration: Duration) {
    let end = Instant::now() + duration;
    while Instant::now() < end {
        bench.ui_alive();
        std::thread::sleep(Duration::from_millis(100));
    }
}

#[test]
fn nothing_can_be_commanded_before_a_driver_is_chosen_and_confirmed() {
    let f = start();
    assert_eq!(f.bench.snapshot().driver, None);
    assert!(matches!(f.bench.arm(true), Err(Error::Safety(_))));
    f.bench.select_driver(DriverKind::Manual).unwrap();
    assert!(matches!(f.bench.arm(true), Err(Error::Safety(_))));
    f.bench.select_driver(DriverKind::NanoPwm).unwrap();
    assert!(matches!(f.bench.arm(false), Err(Error::Safety(_)))); // not confirmed
    assert!(matches!(f.bench.set_throttle(0.2), Err(Error::Safety(_)))); // not armed
    f.bench.arm(true).unwrap();
    wait_for(&f.bench, |s| s.actuator == Some(ActuatorState::Armed));
    f.bench.shutdown();
}

#[test]
fn silent_ui_disarms_the_output() {
    let f = start();
    f.bench.select_driver(DriverKind::NanoPwm).unwrap();
    f.bench.arm(true).unwrap();
    f.bench.set_throttle(0.3).unwrap();
    // No ui_alive() calls from here on: the dead-man must trip (1.5 s).
    wait_for(&f.bench, |s| s.actuator == Some(ActuatorState::Disarmed));
    assert!(f.sink.alarms.lock().unwrap().contains(&"ui_silent".to_string()));
    assert_eq!(f.bench.snapshot().setpoint, 0.0);
    f.bench.shutdown();
}

#[test]
fn estop_latches_until_cleared() {
    let f = start();
    f.bench.select_driver(DriverKind::NanoPwm).unwrap();
    f.bench.arm(true).unwrap();
    f.bench.set_throttle(0.4).unwrap();
    keep_alive(&f.bench, Duration::from_millis(400));
    f.bench.estop();
    let snapshot = wait_for(&f.bench, |s| s.actuator == Some(ActuatorState::EStop));
    assert_eq!(snapshot.setpoint, 0.0);
    assert!(matches!(f.bench.arm(true), Err(Error::Safety(_))));
    f.bench.clear_estop().unwrap();
    wait_for(&f.bench, |s| s.actuator == Some(ActuatorState::Disarmed));
    f.bench.arm(true).unwrap();
    f.bench.shutdown();
}

#[test]
fn thrust_limit_cuts_the_throttle() {
    let f = start();
    let mut settings = f.bench.settings();
    settings.safety.max_thrust_n = Some(2.0);
    settings.safety.max_rise_per_s = 2.0;
    f.bench.update_settings(settings).unwrap();
    f.bench.select_driver(DriverKind::NanoPwm).unwrap();
    f.bench.tare(1).unwrap();
    f.bench.arm(true).unwrap();
    f.bench.set_throttle(0.6).unwrap(); // simulator: 30 N * 0.6^2 = 10.8 N
    keep_alive(&f.bench, Duration::from_millis(1500));
    assert!(f.sink.alarms.lock().unwrap().contains(&"thrust_limit".to_string()));
    wait_for(&f.bench, |s| s.actuator == Some(ActuatorState::Disarmed));
    f.bench.shutdown();
}

#[test]
fn profile_recording_and_curve_analysis() {
    let f = start();
    let mut settings = f.bench.settings();
    settings.safety.max_rise_per_s = 5.0;
    f.bench.update_settings(settings).unwrap();
    f.bench.select_driver(DriverKind::NanoPwm).unwrap();
    f.bench.tare(1).unwrap();
    f.bench.tare(2).unwrap();
    assert!(f.bench.snapshot().tare_age_s.is_some());

    let mut setup = TestSetup::default();
    setup.motor.model = "SimMotor".into();
    let status = f.bench.start_recording(setup).unwrap();
    assert!(f.bench.disconnect().is_err(), "must not disconnect while recording");

    f.bench.arm(true).unwrap();
    let hold = |target| Step { kind: StepKind::Hold, target, duration_s: 1.5 };
    f.bench
        .run_profile(Profile { name: "test".into(), description: String::new(), steps: vec![hold(0.2), hold(0.4), hold(0.0)] })
        .unwrap();
    assert!(matches!(f.bench.set_throttle(0.1), Err(Error::Safety(_))), "manual control is locked during a profile");
    keep_alive(&f.bench, Duration::from_millis(4800));
    assert!(f.bench.snapshot().profile.is_none());
    f.bench.disarm().unwrap();

    let meta = f.bench.stop_recording().unwrap();
    assert_eq!(meta.state, RecordingState::Complete);
    assert_eq!(meta.dropped_rows, 0);
    assert!(meta.rows > 500, "rows: {}", meta.rows);
    assert!(meta.events.iter().any(|e| e.kind == "profile_start"));

    let data = load_recording(&recording_dir(&f.bench.data_dir(), &status.id).unwrap()).unwrap();
    let throttle = &data.by_key("throttle").unwrap().series;
    let thrust = &data.by_key("thrust").unwrap().series;
    let torque = &data.by_key("torque").unwrap().series;
    let curve = throttle_curve(&detect_plateaus(throttle, 0.01, 1.0), thrust, Some(torque), 0.4);
    assert_eq!(curve.len(), 2, "plateaus: {curve:?}");
    // Simulator model: thrust = 30 N * throttle^2, torque = 0.5 N·m * throttle^2.
    assert!((curve[0].thrust_mean - 1.2).abs() < 0.15, "{curve:?}");
    assert!((curve[1].thrust_mean - 4.8).abs() < 0.3, "{curve:?}");
    assert!((curve[1].torque_mean.unwrap() - 0.08).abs() < 0.01, "{curve:?}");
    assert!(*f.sink.samples.lock().unwrap() > 500);
    f.bench.shutdown();
}
