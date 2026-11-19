// Composition root of the firmware: owns every module and runs the
// cooperative, delay-free main loop.
#pragma once

namespace tb {

class DeviceApp {
 public:
  static void setup();
  static void loop();
};

}  // namespace tb
