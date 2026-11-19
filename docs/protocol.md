# Wire protocol, version 1

Serial link between the acquisition board (Arduino Nano) and the PC.

- 500 000 baud, 8N1. Opening the port resets the Nano (DTR); it answers after about 2 s.
- Source of truth for identifiers: `firmware/ThrustBench/src/protocol/Protocol.h`, mirrored in
  `app/crates/tb-core/src/protocol/ids.rs`. Change both and this file together.
- All multi-byte fields are little endian.

## Framing

```
body  = type(1) seq(1) payload(0..44) crc16(2)
wire  = COBS(body) 0x00
```

- `crc16` is CRC-16/CCITT-FALSE (poly 0x1021, init 0xFFFF) over `type..payload`. Check value for "123456789": 0x29B1.
- COBS removes every 0x00 from the body, so 0x00 always marks the end of a frame. A receiver that joins mid-stream
  resynchronises at the next 0x00.
- `seq` increments per frame sent, per direction. The PC counts gaps in the device's sequence as lost frames.
- A body is at most 48 bytes, so an encoded frame fits the Nano's 63-byte transmit buffer and never blocks.

## Device to PC

| Type | Name | Payload |
|---|---|---|
| 0x81 | Info | proto(u8) fwMajor(u8) fwMinor(u8) fwPatch(u8) uid(u32) channelCount(u8) capabilities(u16) defaultsLoaded(u8) |
| 0x82 | Channel | index(u8) count(u8) id(u8) sensorType(u8) unit(u8) flags(u8) rateHz(u16) gain(u8) offset(i32) scale(f32) calTime(u32) name(8 bytes, zero padded) |
| 0x83 | Data | timeUs(u32) n(u8) then n x [channel(u8) raw(i32)] |
| 0x84 | Ack | cmd(u8) cmdSeq(u8) |
| 0x85 | Nack | cmd(u8) cmdSeq(u8) error(u8) |
| 0x86 | Status | state(u8) flags(u8) throttle(u16) pulseUs(u16) txDropped(u16) crcErrors(u16) loopMaxUs(u16) watchdogTrips(u8) uptimeMs(u32) |
| 0x87 | Param | scope(u8) field(u8) value(4 bytes) |

- **Data** carries raw, uncalibrated values. `timeUs` is `micros()` and wraps every 71.6 minutes; the PC unwraps it.
  Data frames are dropped (and counted in `txDropped`) rather than delaying the loop if the transmit buffer is full.
- **Status** is sent every 100 ms. Its absence for 1.5 s is how the PC detects a lost link.
- The engineering value of a channel is `(raw - offset) / scale`, negated when the invert flag is set.
  The device stores this calibration and reports it in **Channel**; the PC applies it.

Channel flags: 0x01 enabled, 0x02 invert, 0x04 calibrated. Status flags: 0x01 streaming, 0x02 config dirty
(RAM differs from EEPROM), 0x04 defaults loaded at boot, 0x08 data frames were dropped.
Capabilities: 0x0001 PWM actuator, 0x0002 HX711 RATE pin wired.

Actuator states: 0 disarmed, 1 armed, 2 emergency stop (latched), 3 failsafe (watchdog tripped).
Throttle is 0..10000 (0.01 % steps).

## PC to device

| Type | Name | Payload | Reply |
|---|---|---|---|
| 0x01 | GetInfo | - | Info |
| 0x02 | GetChannel | index(u8) | Channel |
| 0x03 | SetStream | enabled(u8) | Ack |
| 0x05 | GetParam | scope(u8) field(u8) | Param |
| 0x06 | SetParam | scope(u8) field(u8) value(4 bytes) | Ack |
| 0x07 | SaveConfig | - | Ack (refused while armed) |
| 0x08 | LoadDefaults | - | Ack (RAM only; refused while armed) |
| 0x10 | Heartbeat | - | none |
| 0x11 | Arm | magic(u16) = 0xA55A | Ack |
| 0x12 | Disarm | - | Ack |
| 0x13 | SetThrottle | throttle(u16) | Ack |
| 0x14 | EStop | - | Ack |
| 0x15 | ClearEStop | magic(u16) = 0xA55A | Ack |

Any valid frame from the PC feeds the actuator watchdog. Errors: 1 unknown command, 2 bad length, 3 bad value,
4 not allowed in the current state, 5 EEPROM write failed.

## Parameters

Scope is a channel id (1 thrust, 2 torque) or 0xFF for board-level settings.

| Channel field | Id | Type | Meaning |
|---|---|---|---|
| Enabled | 1 | u32 | 0 or 1 |
| Gain | 2 | u32 | 128 or 64 |
| Offset | 3 | i32 | raw counts at zero load |
| Scale | 4 | f32 | raw counts per unit (N or N·m) |
| Invert | 5 | u32 | 0 or 1 |
| CalTime | 6 | u32 | Unix time of calibration, 0 = nominal |
| SensorType | 7 | u32 | 1 thrust, 2 torque, 3 generic |

| Global field | Id | Range | Default |
|---|---|---|---|
| PwmMinUs | 1 | 500..PwmMax-100 | 1000 |
| PwmMaxUs | 2 | PwmMin+100..2500 | 2000 |
| PwmFreqHz | 3 | 50..490 | 50 |
| WatchdogMs | 4 | 100..2000 | 300 |
| ThrottleMax | 5 | 0..10000 | 6000 |
| SlewPerSec | 6 | 0..65535 (0 = unlimited) | 5000 |
| RatePin | 7 | free digital pin, 255 = not wired | 255 |
| RateSps | 8 | 10 or 80 | 80 |
| DisarmedOutput | 9 | 0 minimum pulse, 1 no signal | 0 |
| DeviceUid | 10 | read only | random per board |

Global fields cannot be written while armed.

## Sensor types and units

| Code | Sensor | | Code | Unit |
|---|---|---|---|---|
| 0x01 | thrust load cell | | 0 | counts |
| 0x02 | torque load cell | | 1 | N |
| 0x03 | generic load cell | | 2 | N·m |
| 0x10-0x12 | acceleration X/Y/Z | | 3 | V |
| 0x20 | voltage | | 4 | A |
| 0x21 | current | | 5 | rpm |
| 0x30 | RPM | | 6 | m/s² |
| 0x40 | throttle command | | 7 | fraction (raw / 10000) |

The PC maps sensor codes to names and units in `app/crates/tb-core/src/config/defaults.json` (`sensor_catalog`),
so a new sensor needs a code here, an entry there and a `Sensor` class in the firmware; nothing else.

## Connection sequence

1. Open the port; wait for the bootloader (the PC retries GetInfo for up to 6 s).
2. GetInfo; refuse to continue if the protocol version differs.
3. GetChannel for every index.
4. SetStream(1). Data and Status now flow; the PC sends Heartbeat whenever it has been silent for 80 ms.

## Compatibility rules

- Adding a sensor type, a parameter or a message type is backwards compatible: unknown ones are rejected or ignored.
- Changing the layout of an existing payload requires a new protocol version.
