#include "DeviceApp.h"

#include <Arduino.h>

#include "../actuators/ThrottleController.h"
#include "../config/BoardPins.h"
#include "../config/ConfigStore.h"
#include "../config/Defaults.h"
#include "../hal/Hx711.h"
#include "../hal/PwmOutput.h"
#include "../protocol/FrameCodec.h"
#include "../sensors/LoadCellSensor.h"
#include "CommandHandler.h"

namespace tb {
namespace {

// All objects are statically allocated: the ATmega328P has 2 KB of RAM and
// no use for a heap.
ConfigStore g_config;

Hx711 g_thrustAdc(pins::kThrustDout, pins::kThrustSck);
Hx711 g_torqueAdc(pins::kTorqueDout, pins::kTorqueSck);
PwmOutput g_pwm;

ThrottleController g_throttle(g_pwm, g_config.data().global);

LoadCellSensor g_thrust(channel::kThrust, "thrust", g_thrustAdc, g_config.data().loadCells[0],
                        g_config.data().global);
LoadCellSensor g_torque(channel::kTorque, "torque", g_torqueAdc, g_config.data().loadCells[1],
                        g_config.data().global);
ThrottleChannel g_throttleChannel(g_throttle);

// Register new sensors here. Order defines the descriptor index only.
Sensor* const g_sensors[] = {&g_thrust, &g_torque, &g_throttleChannel};
constexpr uint8_t kSensorCount = sizeof(g_sensors) / sizeof(g_sensors[0]);

proto::FrameWriter g_writer(Serial);
proto::FrameReader g_reader;
bool g_streaming = false;

uint32_t g_lastStatusMs = 0;
uint16_t g_loopMaxUs = 0;

// Bounds the time one loop pass spends on incoming bytes, so a flood of
// commands cannot starve the sensors.
constexpr uint8_t kMaxRxBytesPerPass = 24;

void applyConfig() {
  const GlobalConfig& g = g_config.data().global;
  if (g.ratePin != 255) {
    pinMode(g.ratePin, OUTPUT);
    digitalWrite(g.ratePin, g.rateSps == 80 ? HIGH : LOW);
  }
  for (uint8_t i = 0; i < kSensorCount; ++i) {
    g_sensors[i]->applyConfig();
  }
  g_throttle.applyConfig();
}

const DeviceContext g_context = {g_writer, g_config, g_throttle, g_sensors,
                                 kSensorCount, g_streaming, &applyConfig};
CommandHandler g_commands(g_context);

void sendSample(const Sample& sample) {
  uint8_t p[10];
  proto::put32(p, sample.timeUs);
  p[4] = 1;  // samples in this frame
  p[5] = sample.channel;
  proto::put32(p + 6, static_cast<uint32_t>(sample.value));
  g_writer.send(proto::Msg::Data, p, sizeof(p), true);
}

void sendStatus(uint32_t nowMs) {
  uint8_t flags = 0;
  if (g_streaming) flags |= proto::kStatusStreaming;
  if (g_config.dirty()) flags |= proto::kStatusConfigDirty;
  if (g_config.defaultsLoadedAtBoot()) flags |= proto::kStatusDefaultsLoaded;
  if (g_writer.dropped() != 0) flags |= proto::kStatusTxOverflow;

  uint8_t p[17];
  p[0] = static_cast<uint8_t>(g_throttle.state());
  p[1] = flags;
  proto::put16(p + 2, g_throttle.throttle());
  proto::put16(p + 4, g_throttle.pulseUs());
  proto::put16(p + 6, g_writer.dropped());
  proto::put16(p + 8, g_reader.errors());
  proto::put16(p + 10, g_loopMaxUs);
  p[12] = g_throttle.watchdogTrips();
  proto::put32(p + 13, nowMs);
  // Droppable: a full buffer must not stall the loop, and the next status
  // frame follows shortly.
  g_writer.send(proto::Msg::Status, p, sizeof(p), true);
  g_loopMaxUs = 0;
}

}  // namespace

void DeviceApp::setup() {
  Serial.begin(defaults::kBaudRate);
  g_config.begin();
  for (uint8_t i = 0; i < kSensorCount; ++i) {
    g_sensors[i]->begin();
  }
  // Starts disarmed, at the minimum pulse (or no signal, per configuration).
  g_throttle.begin(millis());
  applyConfig();
}

void DeviceApp::loop() {
  const uint32_t passStart = micros();
  const uint32_t nowMs = millis();

  // 1. Host commands. Any valid frame feeds the actuator watchdog.
  for (uint8_t n = 0; n < kMaxRxBytesPerPass && Serial.available() > 0; ++n) {
    if (g_reader.feed(static_cast<uint8_t>(Serial.read()))) {
      g_throttle.notifyHostAlive(nowMs);
      g_commands.handle(g_reader.type(), g_reader.seq(), g_reader.payload(),
                        g_reader.payloadLen(), nowMs);
    }
  }

  // 2. Actuator: watchdog and rise-rate limiter.
  g_throttle.update(nowMs);

  // 3. Sensors. Polled even when not streaming so converters keep running.
  Sample sample;
  for (uint8_t i = 0; i < kSensorCount; ++i) {
    if (g_sensors[i]->poll(sample) && g_streaming) {
      sendSample(sample);
    }
  }

  // 4. Periodic status.
  if (nowMs - g_lastStatusMs >= defaults::kStatusPeriodMs) {
    g_lastStatusMs = nowMs;
    sendStatus(nowMs);
  }

  const uint32_t passUs = micros() - passStart;
  if (passUs > g_loopMaxUs) {
    g_loopMaxUs = passUs > 0xFFFF ? 0xFFFF : static_cast<uint16_t>(passUs);
  }
}

}  // namespace tb
