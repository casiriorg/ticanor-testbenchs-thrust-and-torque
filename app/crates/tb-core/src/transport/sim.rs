//! In-process simulated bench.
//!
//! Behaves like the firmware on the other end of the wire: same frames, same
//! actuator state machine, same watchdog. A small motor model turns the
//! commanded throttle into thrust and torque so the whole application can be
//! developed, demonstrated and tested without hardware.
//!
//! The firmware itself can also be built for a PC (firmware/host_sim); that
//! variant runs the real C++ code and is used by the integration test.

use std::collections::{BTreeMap, VecDeque};
use std::io;
use std::time::{Duration, Instant};

use super::Transport;
use crate::domain::{ActuatorState, ChannelDescriptor, DeviceInfo, DeviceStatus};
use crate::protocol::ids::{self, channel_field, global_field, sensor};
use crate::protocol::{encode_frame, Command, DeviceMessage, Frame, FrameDecoder};

#[derive(Debug, Clone, Default)]
pub struct SimOptions {
    /// Simulates an unplugged cable after this long.
    pub disconnect_after: Option<Duration>,
    /// Time the "bootloader" stays silent after opening.
    pub boot_delay: Duration,
}

const ERR_BAD_VALUE: u8 = 3;
const ERR_NOT_ALLOWED: u8 = 4;
const SAMPLE_PERIOD_US: u64 = 12_500; // 80 SPS
const THRUST_FULL_N: f64 = 30.0;
const TORQUE_FULL_NM: f64 = 0.5;

pub struct SimDevice {
    options: SimOptions,
    start: Instant,
    decoder: FrameDecoder,
    out: VecDeque<u8>,
    seq: u8,
    channels: Vec<ChannelDescriptor>,
    globals: BTreeMap<u8, u32>,
    state: ActuatorState,
    target: u16,
    current: f64,
    level: f64,
    streaming: bool,
    config_dirty: bool,
    watchdog_trips: u8,
    last_host_us: u64,
    last_step_us: u64,
    next_sample_us: [u64; 2],
    next_status_us: u64,
    next_throttle_us: u64,
    throttle_sample_pending: bool,
    rng: u64,
}

fn load_cell(id: u8, sensor_type: u8, unit: u8, scale: f32, name: &str) -> ChannelDescriptor {
    ChannelDescriptor {
        id,
        sensor_type,
        unit,
        enabled: true,
        invert: false,
        calibrated: false,
        rate_hz: 80,
        gain: 128,
        offset: 0,
        scale,
        cal_time: 0,
        name: name.to_string(),
    }
}

impl SimDevice {
    pub fn new(options: SimOptions) -> Self {
        let thrust_scale = 21_898.2_f32; // nominal 10 kg bar cell, counts per N
        let channels = vec![
            load_cell(1, sensor::LOAD_CELL_THRUST, 1, thrust_scale, "thrust"),
            load_cell(2, sensor::LOAD_CELL_TORQUE, 2, thrust_scale / 0.040, "torque"),
            ChannelDescriptor {
                id: 16,
                sensor_type: sensor::THROTTLE_COMMAND,
                unit: 7,
                enabled: true,
                invert: false,
                calibrated: true,
                rate_hz: 50,
                gain: 0,
                offset: 0,
                scale: ids::THROTTLE_FULL as f32,
                cal_time: 0,
                name: "throttle".to_string(),
            },
        ];
        let globals = BTreeMap::from([
            (global_field::PWM_MIN_US, 1000),
            (global_field::PWM_MAX_US, 2000),
            (global_field::PWM_FREQ_HZ, 50),
            (global_field::WATCHDOG_MS, 300),
            (global_field::THROTTLE_MAX, 6000),
            (global_field::SLEW_PER_SEC, 5000),
            (global_field::RATE_PIN, 255),
            (global_field::RATE_SPS, 80),
            (global_field::DISARMED_OUTPUT, 0),
            (global_field::DEVICE_UID, 0x5349_4D31), // "SIM1"
        ]);
        let boot_us = options.boot_delay.as_micros() as u64;
        Self {
            options,
            start: Instant::now(),
            decoder: FrameDecoder::new(),
            out: VecDeque::new(),
            seq: 0,
            channels,
            globals,
            state: ActuatorState::Disarmed,
            target: 0,
            current: 0.0,
            level: 0.0,
            streaming: false,
            config_dirty: false,
            watchdog_trips: 0,
            last_host_us: 0,
            last_step_us: 0,
            next_sample_us: [boot_us + 20_000, boot_us + 26_000],
            next_status_us: boot_us + 100_000,
            next_throttle_us: 0,
            throttle_sample_pending: true,
            rng: 0x2545_F491_4F6C_DD1D,
        }
    }

    fn now_us(&self) -> u64 {
        self.start.elapsed().as_micros() as u64
    }

    fn global(&self, field: u8) -> u32 {
        self.globals.get(&field).copied().unwrap_or(0)
    }

    /// Approximately normal noise from the sum of uniform samples.
    fn noise(&mut self, sigma: f64) -> f64 {
        let mut sum = 0.0;
        for _ in 0..4 {
            self.rng ^= self.rng << 13;
            self.rng ^= self.rng >> 7;
            self.rng ^= self.rng << 17;
            sum += (self.rng >> 11) as f64 / (1u64 << 53) as f64 - 0.5;
        }
        sum * sigma * 1.73
    }

    fn send(&mut self, message: DeviceMessage) {
        if let Ok(wire) = encode_frame(message.kind(), self.seq, &message.payload()) {
            self.seq = self.seq.wrapping_add(1);
            self.out.extend(wire);
        }
    }

    fn force_zero(&mut self, next: ActuatorState) {
        self.state = next;
        self.target = 0;
        self.current = 0.0;
        self.throttle_sample_pending = true;
    }

    /// Advances the actuator and motor model to `now`.
    fn step(&mut self, now: u64) {
        let dt = (now.saturating_sub(self.last_step_us)) as f64 * 1e-6;
        self.last_step_us = now;
        if self.state == ActuatorState::Armed {
            let watchdog_us = self.global(global_field::WATCHDOG_MS) as u64 * 1000;
            if now.saturating_sub(self.last_host_us) > watchdog_us {
                self.watchdog_trips = self.watchdog_trips.saturating_add(1);
                self.force_zero(ActuatorState::Failsafe);
            } else {
                let target = self.target as f64;
                let slew = self.global(global_field::SLEW_PER_SEC) as f64;
                self.current = if target < self.current || slew == 0.0 {
                    target
                } else {
                    (self.current + slew * dt).min(target)
                };
            }
        }
        // First-order lag between the electrical command and the propeller.
        let command = self.current / ids::THROTTLE_FULL as f64;
        self.level += (command - self.level) * (1.0 - (-dt / 0.08).exp());
    }

    fn emit_due(&mut self, now: u64) {
        for index in 0..2 {
            while self.next_sample_us[index] <= now {
                let t = self.next_sample_us[index];
                self.next_sample_us[index] += SAMPLE_PERIOD_US;
                self.step(t);
                let (physical, bias) = if index == 0 {
                    let ripple = 0.05 * self.level * (2.0 * std::f64::consts::PI * 17.0 * t as f64 * 1e-6).sin();
                    (THRUST_FULL_N * self.level * self.level + ripple, 84_210.0)
                } else {
                    (TORQUE_FULL_NM * self.level * self.level, -35_120.0)
                };
                let channel = &self.channels[index];
                let (id, enabled, scale) = (channel.id, channel.enabled, channel.scale as f64);
                let raw = (bias + physical * scale + self.noise(40.0)).round() as i32;
                if self.streaming && enabled {
                    self.send(DeviceMessage::Data { time_us: t as u32, samples: vec![(id, raw)] });
                }
            }
        }
        self.step(now);
        let periodic = self.state == ActuatorState::Armed && now >= self.next_throttle_us;
        if self.throttle_sample_pending || periodic {
            self.throttle_sample_pending = false;
            self.next_throttle_us = now + 20_000;
            if self.streaming {
                self.send(DeviceMessage::Data { time_us: now as u32, samples: vec![(16, self.current.round() as i32)] });
            }
        }
        if now >= self.next_status_us {
            self.next_status_us = now + 100_000;
            let span = (self.global(global_field::PWM_MAX_US) - self.global(global_field::PWM_MIN_US)) as f64;
            let status = DeviceStatus {
                actuator: self.state,
                streaming: self.streaming,
                config_dirty: self.config_dirty,
                defaults_loaded: false,
                tx_overflow: false,
                throttle: self.current / ids::THROTTLE_FULL as f64,
                pulse_us: (self.global(global_field::PWM_MIN_US) as f64 + span * self.current / ids::THROTTLE_FULL as f64) as u16,
                tx_dropped: 0,
                crc_errors: self.decoder.errors() as u16,
                loop_max_us: 180,
                watchdog_trips: self.watchdog_trips,
                uptime_ms: (now / 1000) as u32,
            };
            self.send(DeviceMessage::Status(status));
        }
    }

    fn set_param(&mut self, scope: u8, field: u8, value: [u8; 4]) -> Result<(), u8> {
        let raw = u32::from_le_bytes(value);
        if scope == ids::SCOPE_GLOBAL {
            if self.state == ActuatorState::Armed || field == global_field::DEVICE_UID {
                return Err(ERR_NOT_ALLOWED);
            }
            if !self.globals.contains_key(&field) {
                return Err(ERR_BAD_VALUE);
            }
            if field == global_field::THROTTLE_MAX && raw > ids::THROTTLE_FULL as u32 {
                return Err(ERR_BAD_VALUE);
            }
            self.globals.insert(field, raw);
            return Ok(());
        }
        let channel = self.channels.iter_mut().find(|c| c.id == scope && c.id != 16).ok_or(ERR_BAD_VALUE)?;
        match field {
            channel_field::ENABLED => channel.enabled = raw != 0,
            channel_field::GAIN if raw == 128 || raw == 64 => channel.gain = raw as u8,
            channel_field::OFFSET => channel.offset = raw as i32,
            channel_field::SCALE => {
                let scale = f32::from_le_bytes(value);
                if !(scale > 1e-6 && scale < 1e12) {
                    return Err(ERR_BAD_VALUE);
                }
                channel.scale = scale;
            }
            channel_field::INVERT => channel.invert = raw != 0,
            channel_field::CAL_TIME => {
                channel.cal_time = raw;
                channel.calibrated = raw != 0;
            }
            channel_field::SENSOR_TYPE if (1..=3).contains(&raw) => channel.sensor_type = raw as u8,
            _ => return Err(ERR_BAD_VALUE),
        }
        Ok(())
    }

    fn get_param(&self, scope: u8, field: u8) -> Option<[u8; 4]> {
        if scope == ids::SCOPE_GLOBAL {
            return self.globals.get(&field).map(|v| v.to_le_bytes());
        }
        let c = self.channels.iter().find(|c| c.id == scope)?;
        Some(match field {
            channel_field::ENABLED => (c.enabled as u32).to_le_bytes(),
            channel_field::GAIN => (c.gain as u32).to_le_bytes(),
            channel_field::OFFSET => c.offset.to_le_bytes(),
            channel_field::SCALE => c.scale.to_le_bytes(),
            channel_field::INVERT => (c.invert as u32).to_le_bytes(),
            channel_field::CAL_TIME => c.cal_time.to_le_bytes(),
            channel_field::SENSOR_TYPE => (c.sensor_type as u32).to_le_bytes(),
            _ => return None,
        })
    }

    fn handle(&mut self, frame: Frame, now: u64) {
        self.last_host_us = now;
        let (kind, seq) = (frame.kind, frame.seq);
        let command = match Command::parse(&frame) {
            Ok(command) => command,
            Err(_) => return self.send(DeviceMessage::Nack { cmd: kind, seq, error: 1 }),
        };
        let outcome: Result<(), u8> = match command {
            Command::GetInfo => {
                let info = DeviceInfo {
                    protocol_version: ids::PROTOCOL_VERSION,
                    firmware: "0.1.0".into(),
                    uid: self.global(global_field::DEVICE_UID),
                    channel_count: self.channels.len() as u8,
                    capabilities: ids::capability::PWM_ACTUATOR,
                    defaults_loaded: false,
                };
                return self.send(DeviceMessage::Info(info));
            }
            Command::GetChannel(index) => match self.channels.get(index as usize).cloned() {
                Some(descriptor) => {
                    let count = self.channels.len() as u8;
                    return self.send(DeviceMessage::Channel { index, count, descriptor });
                }
                None => Err(ERR_BAD_VALUE),
            },
            Command::GetParam { scope, field } => match self.get_param(scope, field) {
                Some(value) => return self.send(DeviceMessage::Param { scope, field, value }),
                None => Err(ERR_BAD_VALUE),
            },
            Command::Heartbeat => return,
            Command::SetStream(on) => {
                self.streaming = on;
                Ok(())
            }
            Command::SetParam { scope, field, value } => self.set_param(scope, field, value).map(|_| self.config_dirty = true),
            Command::SaveConfig | Command::LoadDefaults if self.state == ActuatorState::Armed => Err(ERR_NOT_ALLOWED),
            Command::SaveConfig => {
                self.config_dirty = false;
                Ok(())
            }
            Command::LoadDefaults => {
                let fresh = SimDevice::new(SimOptions::default());
                self.channels = fresh.channels;
                self.globals = fresh.globals;
                self.config_dirty = true;
                Ok(())
            }
            Command::Arm if self.state == ActuatorState::Disarmed => {
                self.force_zero(ActuatorState::Armed);
                Ok(())
            }
            Command::Arm => Err(ERR_NOT_ALLOWED),
            Command::Disarm => {
                if self.state != ActuatorState::EStop {
                    self.force_zero(ActuatorState::Disarmed);
                }
                Ok(())
            }
            Command::SetThrottle(value) if self.state == ActuatorState::Armed && value <= ids::THROTTLE_FULL => {
                self.target = value.min(self.global(global_field::THROTTLE_MAX) as u16);
                Ok(())
            }
            Command::SetThrottle(_) => Err(ERR_NOT_ALLOWED),
            Command::EStop => {
                self.force_zero(ActuatorState::EStop);
                Ok(())
            }
            Command::ClearEStop if self.state == ActuatorState::EStop => {
                self.force_zero(ActuatorState::Disarmed);
                Ok(())
            }
            Command::ClearEStop => Err(ERR_NOT_ALLOWED),
        };
        match outcome {
            Ok(()) => self.send(DeviceMessage::Ack { cmd: kind, seq }),
            Err(error) => self.send(DeviceMessage::Nack { cmd: kind, seq, error }),
        }
    }

    fn unplugged(&self) -> bool {
        self.options.disconnect_after.is_some_and(|after| self.start.elapsed() >= after)
    }
}

impl Transport for SimDevice {
    fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
        if self.unplugged() {
            return Err(io::Error::new(io::ErrorKind::BrokenPipe, "simulated disconnect"));
        }
        let now = self.now_us();
        if self.start.elapsed() >= self.options.boot_delay {
            self.emit_due(now);
        }
        if self.out.is_empty() {
            std::thread::sleep(Duration::from_millis(2));
            return Ok(0);
        }
        let n = buf.len().min(self.out.len());
        for (slot, byte) in buf.iter_mut().zip(self.out.drain(..n)) {
            *slot = byte;
        }
        Ok(n)
    }

    fn write_all(&mut self, data: &[u8]) -> io::Result<()> {
        if self.unplugged() {
            return Err(io::Error::new(io::ErrorKind::BrokenPipe, "simulated disconnect"));
        }
        if self.start.elapsed() < self.options.boot_delay {
            return Ok(()); // the bootloader discards what it receives
        }
        let mut frames = Vec::new();
        self.decoder.push(data, &mut frames);
        let now = self.now_us();
        self.step(now);
        for frame in frames {
            self.handle(frame, now);
        }
        Ok(())
    }

    fn label(&self) -> String {
        "simulator".to_string()
    }
}
