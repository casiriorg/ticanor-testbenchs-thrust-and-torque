// Entry point of the host-built firmware. Runs the real setup()/loop() until
// the host closes stdin.
#include <chrono>
#include <thread>

#include "Arduino.h"

#include "../ThrustBench/src/app/DeviceApp.h"

int main() {
  tb::DeviceApp::setup();
  while (!Serial.closed()) {
    tb::DeviceApp::loop();
    // The real loop spins freely; yield here so the simulation does not peg a core.
    std::this_thread::sleep_for(std::chrono::microseconds(100));
  }
  return 0;
}
