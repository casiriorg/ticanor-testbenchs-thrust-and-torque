// Frame writer and reader on top of an Arduino Print / byte stream.
#pragma once

#include <Arduino.h>

#include "Protocol.h"

namespace tb {
namespace proto {

// Little-endian packing helpers. memcpy keeps them alignment safe.
inline void put16(uint8_t* p, uint16_t v) { memcpy(p, &v, 2); }
inline void put32(uint8_t* p, uint32_t v) { memcpy(p, &v, 4); }
inline void putFloat(uint8_t* p, float v) { memcpy(p, &v, 4); }
inline uint16_t get16(const uint8_t* p) { uint16_t v; memcpy(&v, p, 2); return v; }
inline uint32_t get32(const uint8_t* p) { uint32_t v; memcpy(&v, p, 4); return v; }
inline float getFloat(const uint8_t* p) { float v; memcpy(&v, p, 4); return v; }

class FrameWriter {
 public:
  explicit FrameWriter(Print& out) : out_(out) {}

  // Sends a frame. With `droppable` set, the frame is discarded instead of
  // blocking when the transmit buffer has no room; this keeps the acquisition
  // loop free of delays. Replies to commands are never droppable.
  bool send(Msg type, const uint8_t* payload, uint8_t len, bool droppable);

  uint16_t dropped() const { return dropped_; }

 private:
  Print& out_;
  uint8_t seq_ = 0;
  uint16_t dropped_ = 0;
};

// Accumulates bytes until a 0x00 delimiter, then validates the frame.
class FrameReader {
 public:
  // Feeds one received byte. Returns true when a complete, CRC-valid frame is
  // available through the accessors below (valid until the next feed()).
  bool feed(uint8_t byte);

  uint8_t type() const { return buf_[0]; }
  uint8_t seq() const { return buf_[1]; }
  const uint8_t* payload() const { return buf_ + 2; }
  uint8_t payloadLen() const { return payloadLen_; }
  uint16_t errors() const { return errors_; }

 private:
  uint8_t buf_[kMaxEncoded];
  uint8_t len_ = 0;
  uint8_t payloadLen_ = 0;
  bool overflow_ = false;
  uint16_t errors_ = 0;
};

}  // namespace proto
}  // namespace tb
