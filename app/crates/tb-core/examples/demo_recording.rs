//! Records a staircase test with the built-in simulator, without hardware.
//! Useful to try the recordings browser and the Python notebooks.
//!
//! ```text
//! cargo run -p tb-core --example demo_recording -- <recordings-folder> [motor-name]
//! ```

use std::sync::Arc;
use std::time::{Duration, Instant};

use tb_core::domain::DriverKind;
use tb_core::profile::Profile;
use tb_core::recording::TestSetup;
use tb_core::services::bench::{Bench, BenchConfig, BenchEvent, ConnectionState, EventSink};

struct Quiet;

impl EventSink for Quiet {
    fn emit(&self, _event: BenchEvent) {}
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let mut args = std::env::args().skip(1);
    let root = args.next().ok_or("usage: demo_recording <recordings-folder> [motor-name]")?;
    let motor = args.next().unwrap_or_else(|| "SimMotor".to_string());
    let scratch = std::env::temp_dir().join("thrustbench-demo-config");
    std::fs::create_dir_all(&scratch)?;
    // Keep the demo away from real serial ports.
    std::fs::write(scratch.join("settings.json"), r#"{"auto_connect": false, "safety": {"max_rise_per_s": 2.0}}"#)?;

    let bench = Bench::start(
        BenchConfig { config_dir: scratch, default_data_dir: root.into(), app_version: env!("CARGO_PKG_VERSION").into() },
        Arc::new(Quiet),
    );
    bench.connect_simulator()?;
    let deadline = Instant::now() + Duration::from_secs(10);
    while !matches!(bench.snapshot().connection, ConnectionState::Connected { .. }) || bench.snapshot().status.is_none() {
        if Instant::now() > deadline {
            return Err("the simulator did not connect".into());
        }
        std::thread::sleep(Duration::from_millis(50));
    }
    bench.select_driver(DriverKind::NanoPwm)?;
    bench.tare(1)?;
    bench.tare(2)?;

    let mut setup = TestSetup::default();
    setup.motor.brand = "Demo".into();
    setup.motor.model = motor;
    setup.propeller.brand = "Sim".into();
    setup.propeller.diameter_in = Some(16.0);
    setup.propeller.pitch_in = Some(5.4);
    setup.test.notes = "Simulated staircase".into();
    let status = bench.start_recording(setup)?;

    bench.arm(true)?;
    let profile = bench.builtin_profiles().into_iter().next().ok_or("no built-in profile")?;
    let total = Profile::total_duration(&profile);
    bench.run_profile(profile)?;
    let end = Instant::now() + Duration::from_secs_f64(total + 0.5);
    while Instant::now() < end {
        bench.ui_alive(); // what the window does while the operator watches
        std::thread::sleep(Duration::from_millis(100));
    }
    bench.disarm()?;
    let meta = bench.stop_recording()?;
    bench.shutdown();
    println!("{} ({} rows, {:.1} s)", status.path, meta.rows, meta.duration_s);
    Ok(())
}
