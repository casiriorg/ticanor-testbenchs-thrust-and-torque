//! Wire identifiers. Mirror of `firmware/ThrustBench/src/protocol/Protocol.h`;
//! the specification is `docs/protocol.md`. Change all three together.

pub const PROTOCOL_VERSION: u8 = 1;
pub const MAX_BODY: usize = 48;
pub const MAX_PAYLOAD: usize = MAX_BODY - 4;
pub const BAUD_RATE: u32 = 500_000;

pub const ARM_MAGIC: u16 = 0xA55A;
pub const THROTTLE_FULL: u16 = 10_000;
pub const SCOPE_GLOBAL: u8 = 0xFF;
pub const CHANNEL_NAME_LEN: usize = 8;

/// Host -> device command codes.
pub mod cmd {
    pub const GET_INFO: u8 = 0x01;
    pub const GET_CHANNEL: u8 = 0x02;
    pub const SET_STREAM: u8 = 0x03;
    pub const GET_PARAM: u8 = 0x05;
    pub const SET_PARAM: u8 = 0x06;
    pub const SAVE_CONFIG: u8 = 0x07;
    pub const LOAD_DEFAULTS: u8 = 0x08;
    pub const HEARTBEAT: u8 = 0x10;
    pub const ARM: u8 = 0x11;
    pub const DISARM: u8 = 0x12;
    pub const SET_THROTTLE: u8 = 0x13;
    pub const ESTOP: u8 = 0x14;
    pub const CLEAR_ESTOP: u8 = 0x15;
}

/// Device -> host message codes.
pub mod msg {
    pub const INFO: u8 = 0x81;
    pub const CHANNEL: u8 = 0x82;
    pub const DATA: u8 = 0x83;
    pub const ACK: u8 = 0x84;
    pub const NACK: u8 = 0x85;
    pub const STATUS: u8 = 0x86;
    pub const PARAM: u8 = 0x87;
}

/// Sensor type codes carried in channel descriptors.
pub mod sensor {
    pub const LOAD_CELL_THRUST: u8 = 0x01;
    pub const LOAD_CELL_TORQUE: u8 = 0x02;
    pub const LOAD_CELL_GENERIC: u8 = 0x03;
    pub const ACCEL_X: u8 = 0x10;
    pub const ACCEL_Y: u8 = 0x11;
    pub const ACCEL_Z: u8 = 0x12;
    pub const VOLTAGE: u8 = 0x20;
    pub const CURRENT: u8 = 0x21;
    pub const RPM: u8 = 0x30;
    pub const THROTTLE_COMMAND: u8 = 0x40;
}

pub mod channel_flag {
    pub const ENABLED: u8 = 0x01;
    pub const INVERT: u8 = 0x02;
    pub const CALIBRATED: u8 = 0x04;
}

pub mod status_flag {
    pub const STREAMING: u8 = 0x01;
    pub const CONFIG_DIRTY: u8 = 0x02;
    pub const DEFAULTS_LOADED: u8 = 0x04;
    pub const TX_OVERFLOW: u8 = 0x08;
}

pub mod capability {
    pub const PWM_ACTUATOR: u16 = 0x0001;
    pub const RATE_PIN: u16 = 0x0002;
}

/// Per-channel parameter fields (scope = channel id).
pub mod channel_field {
    pub const ENABLED: u8 = 1;
    pub const GAIN: u8 = 2;
    pub const OFFSET: u8 = 3;
    pub const SCALE: u8 = 4;
    pub const INVERT: u8 = 5;
    pub const CAL_TIME: u8 = 6;
    pub const SENSOR_TYPE: u8 = 7;
}

/// Global parameter fields (scope = SCOPE_GLOBAL).
pub mod global_field {
    pub const PWM_MIN_US: u8 = 1;
    pub const PWM_MAX_US: u8 = 2;
    pub const PWM_FREQ_HZ: u8 = 3;
    pub const WATCHDOG_MS: u8 = 4;
    pub const THROTTLE_MAX: u8 = 5;
    pub const SLEW_PER_SEC: u8 = 6;
    pub const RATE_PIN: u8 = 7;
    pub const RATE_SPS: u8 = 8;
    pub const DISARMED_OUTPUT: u8 = 9;
    pub const DEVICE_UID: u8 = 10;
}
