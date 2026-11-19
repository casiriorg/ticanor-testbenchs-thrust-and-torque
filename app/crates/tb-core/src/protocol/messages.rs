//! Typed view of the frames. Both directions can be encoded and parsed so
//! the in-process simulator and the tests share the exact same code as the
//! real link.

use super::framing::Frame;
use super::ids::{self, channel_flag, cmd, msg, status_flag};
use crate::domain::{ActuatorState, ChannelDescriptor, DeviceInfo, DeviceStatus};
use crate::error::{Error, Result};

/// Host -> device.
#[derive(Debug, Clone, PartialEq)]
pub enum Command {
    GetInfo,
    GetChannel(u8),
    SetStream(bool),
    GetParam { scope: u8, field: u8 },
    SetParam { scope: u8, field: u8, value: [u8; 4] },
    SaveConfig,
    LoadDefaults,
    Heartbeat,
    Arm,
    Disarm,
    /// Throttle in device units, 0..=THROTTLE_FULL.
    SetThrottle(u16),
    EStop,
    ClearEStop,
}

impl Command {
    pub fn kind(&self) -> u8 {
        match self {
            Command::GetInfo => cmd::GET_INFO,
            Command::GetChannel(_) => cmd::GET_CHANNEL,
            Command::SetStream(_) => cmd::SET_STREAM,
            Command::GetParam { .. } => cmd::GET_PARAM,
            Command::SetParam { .. } => cmd::SET_PARAM,
            Command::SaveConfig => cmd::SAVE_CONFIG,
            Command::LoadDefaults => cmd::LOAD_DEFAULTS,
            Command::Heartbeat => cmd::HEARTBEAT,
            Command::Arm => cmd::ARM,
            Command::Disarm => cmd::DISARM,
            Command::SetThrottle(_) => cmd::SET_THROTTLE,
            Command::EStop => cmd::ESTOP,
            Command::ClearEStop => cmd::CLEAR_ESTOP,
        }
    }

    pub fn payload(&self) -> Vec<u8> {
        match self {
            Command::GetChannel(index) => vec![*index],
            Command::SetStream(on) => vec![*on as u8],
            Command::GetParam { scope, field } => vec![*scope, *field],
            Command::SetParam { scope, field, value } => {
                let mut p = vec![*scope, *field];
                p.extend_from_slice(value);
                p
            }
            Command::Arm | Command::ClearEStop => ids::ARM_MAGIC.to_le_bytes().to_vec(),
            Command::SetThrottle(value) => value.to_le_bytes().to_vec(),
            _ => Vec::new(),
        }
    }

    pub fn parse(frame: &Frame) -> Result<Self> {
        let p = &frame.payload;
        let need = |n: usize| -> Result<()> {
            if p.len() == n {
                Ok(())
            } else {
                Err(Error::Protocol(format!("command 0x{:02X}: bad length {}", frame.kind, p.len())))
            }
        };
        Ok(match frame.kind {
            cmd::GET_INFO => Command::GetInfo,
            cmd::GET_CHANNEL => {
                need(1)?;
                Command::GetChannel(p[0])
            }
            cmd::SET_STREAM => {
                need(1)?;
                Command::SetStream(p[0] != 0)
            }
            cmd::GET_PARAM => {
                need(2)?;
                Command::GetParam { scope: p[0], field: p[1] }
            }
            cmd::SET_PARAM => {
                need(6)?;
                Command::SetParam { scope: p[0], field: p[1], value: [p[2], p[3], p[4], p[5]] }
            }
            cmd::SAVE_CONFIG => Command::SaveConfig,
            cmd::LOAD_DEFAULTS => Command::LoadDefaults,
            cmd::HEARTBEAT => Command::Heartbeat,
            cmd::ARM | cmd::CLEAR_ESTOP => {
                need(2)?;
                if u16::from_le_bytes([p[0], p[1]]) != ids::ARM_MAGIC {
                    return Err(Error::Protocol("bad arming magic".into()));
                }
                if frame.kind == cmd::ARM {
                    Command::Arm
                } else {
                    Command::ClearEStop
                }
            }
            cmd::DISARM => Command::Disarm,
            cmd::SET_THROTTLE => {
                need(2)?;
                Command::SetThrottle(u16::from_le_bytes([p[0], p[1]]))
            }
            cmd::ESTOP => Command::EStop,
            other => return Err(Error::Protocol(format!("unknown command 0x{other:02X}"))),
        })
    }
}

/// Device -> host.
#[derive(Debug, Clone, PartialEq)]
pub enum DeviceMessage {
    Info(DeviceInfo),
    Channel { index: u8, count: u8, descriptor: ChannelDescriptor },
    /// Raw samples sharing one device timestamp (microseconds, wraps at 2^32).
    Data { time_us: u32, samples: Vec<(u8, i32)> },
    Ack { cmd: u8, seq: u8 },
    Nack { cmd: u8, seq: u8, error: u8 },
    Status(DeviceStatus),
    Param { scope: u8, field: u8, value: [u8; 4] },
}

fn u16_at(p: &[u8], i: usize) -> u16 {
    u16::from_le_bytes([p[i], p[i + 1]])
}

fn u32_at(p: &[u8], i: usize) -> u32 {
    u32::from_le_bytes([p[i], p[i + 1], p[i + 2], p[i + 3]])
}

impl DeviceMessage {
    pub fn kind(&self) -> u8 {
        match self {
            DeviceMessage::Info(_) => msg::INFO,
            DeviceMessage::Channel { .. } => msg::CHANNEL,
            DeviceMessage::Data { .. } => msg::DATA,
            DeviceMessage::Ack { .. } => msg::ACK,
            DeviceMessage::Nack { .. } => msg::NACK,
            DeviceMessage::Status(_) => msg::STATUS,
            DeviceMessage::Param { .. } => msg::PARAM,
        }
    }

    pub fn parse(frame: &Frame) -> Result<Self> {
        let p = &frame.payload;
        let short = || Error::Protocol(format!("message 0x{:02X}: payload too short ({})", frame.kind, p.len()));
        Ok(match frame.kind {
            msg::INFO => {
                if p.len() < 12 {
                    return Err(short());
                }
                DeviceMessage::Info(DeviceInfo {
                    protocol_version: p[0],
                    firmware: format!("{}.{}.{}", p[1], p[2], p[3]),
                    uid: u32_at(p, 4),
                    channel_count: p[8],
                    capabilities: u16_at(p, 9),
                    defaults_loaded: p[11] != 0,
                })
            }
            msg::CHANNEL => {
                if p.len() < 21 + ids::CHANNEL_NAME_LEN {
                    return Err(short());
                }
                let name_bytes = &p[21..21 + ids::CHANNEL_NAME_LEN];
                let name_len = name_bytes.iter().position(|&b| b == 0).unwrap_or(name_bytes.len());
                let flags = p[5];
                DeviceMessage::Channel {
                    index: p[0],
                    count: p[1],
                    descriptor: ChannelDescriptor {
                        id: p[2],
                        sensor_type: p[3],
                        unit: p[4],
                        enabled: flags & channel_flag::ENABLED != 0,
                        invert: flags & channel_flag::INVERT != 0,
                        calibrated: flags & channel_flag::CALIBRATED != 0,
                        rate_hz: u16_at(p, 6),
                        gain: p[8],
                        offset: u32_at(p, 9) as i32,
                        scale: f32::from_le_bytes([p[13], p[14], p[15], p[16]]),
                        cal_time: u32_at(p, 17),
                        name: String::from_utf8_lossy(&name_bytes[..name_len]).into_owned(),
                    },
                }
            }
            msg::DATA => {
                if p.len() < 5 {
                    return Err(short());
                }
                let count = p[4] as usize;
                if p.len() != 5 + count * 5 {
                    return Err(Error::Protocol("data frame length does not match its sample count".into()));
                }
                let samples = (0..count)
                    .map(|i| {
                        let base = 5 + i * 5;
                        (p[base], u32_at(p, base + 1) as i32)
                    })
                    .collect();
                DeviceMessage::Data { time_us: u32_at(p, 0), samples }
            }
            msg::ACK => {
                if p.len() < 2 {
                    return Err(short());
                }
                DeviceMessage::Ack { cmd: p[0], seq: p[1] }
            }
            msg::NACK => {
                if p.len() < 3 {
                    return Err(short());
                }
                DeviceMessage::Nack { cmd: p[0], seq: p[1], error: p[2] }
            }
            msg::STATUS => {
                if p.len() < 17 {
                    return Err(short());
                }
                let actuator = ActuatorState::from_code(p[0])
                    .ok_or_else(|| Error::Protocol(format!("unknown actuator state {}", p[0])))?;
                let flags = p[1];
                DeviceMessage::Status(DeviceStatus {
                    actuator,
                    streaming: flags & status_flag::STREAMING != 0,
                    config_dirty: flags & status_flag::CONFIG_DIRTY != 0,
                    defaults_loaded: flags & status_flag::DEFAULTS_LOADED != 0,
                    tx_overflow: flags & status_flag::TX_OVERFLOW != 0,
                    throttle: u16_at(p, 2) as f64 / ids::THROTTLE_FULL as f64,
                    pulse_us: u16_at(p, 4),
                    tx_dropped: u16_at(p, 6),
                    crc_errors: u16_at(p, 8),
                    loop_max_us: u16_at(p, 10),
                    watchdog_trips: p[12],
                    uptime_ms: u32_at(p, 13),
                })
            }
            msg::PARAM => {
                if p.len() < 6 {
                    return Err(short());
                }
                DeviceMessage::Param { scope: p[0], field: p[1], value: [p[2], p[3], p[4], p[5]] }
            }
            other => return Err(Error::Protocol(format!("unknown message 0x{other:02X}"))),
        })
    }

    /// Device-side encoding, used by the simulator.
    pub fn payload(&self) -> Vec<u8> {
        match self {
            DeviceMessage::Info(info) => {
                let mut version = info.firmware.split('.').map(|s| s.parse::<u8>().unwrap_or(0));
                let mut p = vec![info.protocol_version];
                for _ in 0..3 {
                    p.push(version.next().unwrap_or(0));
                }
                p.extend_from_slice(&info.uid.to_le_bytes());
                p.push(info.channel_count);
                p.extend_from_slice(&info.capabilities.to_le_bytes());
                p.push(info.defaults_loaded as u8);
                p
            }
            DeviceMessage::Channel { index, count, descriptor: d } => {
                let flags = (d.enabled as u8 * channel_flag::ENABLED)
                    | (d.invert as u8 * channel_flag::INVERT)
                    | (d.calibrated as u8 * channel_flag::CALIBRATED);
                let mut p = vec![*index, *count, d.id, d.sensor_type, d.unit, flags];
                p.extend_from_slice(&d.rate_hz.to_le_bytes());
                p.push(d.gain);
                p.extend_from_slice(&d.offset.to_le_bytes());
                p.extend_from_slice(&d.scale.to_le_bytes());
                p.extend_from_slice(&d.cal_time.to_le_bytes());
                let mut name = [0u8; ids::CHANNEL_NAME_LEN];
                let bytes = d.name.as_bytes();
                let n = bytes.len().min(name.len());
                name[..n].copy_from_slice(&bytes[..n]);
                p.extend_from_slice(&name);
                p
            }
            DeviceMessage::Data { time_us, samples } => {
                let mut p = time_us.to_le_bytes().to_vec();
                p.push(samples.len() as u8);
                for (channel, value) in samples {
                    p.push(*channel);
                    p.extend_from_slice(&value.to_le_bytes());
                }
                p
            }
            DeviceMessage::Ack { cmd, seq } => vec![*cmd, *seq],
            DeviceMessage::Nack { cmd, seq, error } => vec![*cmd, *seq, *error],
            DeviceMessage::Status(s) => {
                let flags = (s.streaming as u8 * status_flag::STREAMING)
                    | (s.config_dirty as u8 * status_flag::CONFIG_DIRTY)
                    | (s.defaults_loaded as u8 * status_flag::DEFAULTS_LOADED)
                    | (s.tx_overflow as u8 * status_flag::TX_OVERFLOW);
                let mut p = vec![s.actuator.code(), flags];
                p.extend_from_slice(&((s.throttle * ids::THROTTLE_FULL as f64).round() as u16).to_le_bytes());
                p.extend_from_slice(&s.pulse_us.to_le_bytes());
                p.extend_from_slice(&s.tx_dropped.to_le_bytes());
                p.extend_from_slice(&s.crc_errors.to_le_bytes());
                p.extend_from_slice(&s.loop_max_us.to_le_bytes());
                p.push(s.watchdog_trips);
                p.extend_from_slice(&s.uptime_ms.to_le_bytes());
                p
            }
            DeviceMessage::Param { scope, field, value } => {
                let mut p = vec![*scope, *field];
                p.extend_from_slice(value);
                p
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::protocol::framing::{encode_frame, FrameDecoder};

    fn round_trip(message: DeviceMessage) {
        let wire = encode_frame(message.kind(), 3, &message.payload()).unwrap();
        let mut frames = Vec::new();
        FrameDecoder::new().push(&wire, &mut frames);
        assert_eq!(DeviceMessage::parse(&frames[0]).unwrap(), message);
    }

    #[test]
    fn device_messages_round_trip() {
        round_trip(DeviceMessage::Info(DeviceInfo {
            protocol_version: 1,
            firmware: "0.1.0".into(),
            uid: 0xDEADBEEF,
            channel_count: 3,
            capabilities: 1,
            defaults_loaded: true,
        }));
        round_trip(DeviceMessage::Channel {
            index: 1,
            count: 3,
            descriptor: ChannelDescriptor {
                id: 2,
                sensor_type: ids::sensor::LOAD_CELL_TORQUE,
                unit: 2,
                enabled: true,
                invert: true,
                calibrated: false,
                rate_hz: 80,
                gain: 128,
                offset: -1234,
                scale: 547_450.0,
                cal_time: 0,
                name: "torque".into(),
            },
        });
        round_trip(DeviceMessage::Data { time_us: 4_000_000_000, samples: vec![(1, -5), (2, 70_000)] });
        round_trip(DeviceMessage::Nack { cmd: cmd::ARM, seq: 9, error: 4 });
    }

    #[test]
    fn commands_round_trip() {
        for command in [
            Command::GetInfo,
            Command::GetChannel(2),
            Command::SetStream(true),
            Command::SetParam { scope: 1, field: 4, value: 21_898.0f32.to_le_bytes() },
            Command::Arm,
            Command::SetThrottle(4321),
            Command::EStop,
            Command::ClearEStop,
        ] {
            let wire = encode_frame(command.kind(), 0, &command.payload()).unwrap();
            let mut frames = Vec::new();
            FrameDecoder::new().push(&wire, &mut frames);
            assert_eq!(Command::parse(&frames[0]).unwrap(), command);
        }
    }
}
