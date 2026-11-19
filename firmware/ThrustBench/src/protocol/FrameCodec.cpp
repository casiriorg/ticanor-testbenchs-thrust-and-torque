#include "FrameCodec.h"

#include "Cobs.h"
#include "Crc16.h"

namespace tb {
namespace proto {

bool FrameWriter::send(Msg type, const uint8_t* payload, uint8_t len, bool droppable) {
  if (len > kMaxPayload) {
    return false;
  }
  uint8_t body[kMaxBody];
  body[0] = static_cast<uint8_t>(type);
  body[1] = seq_;
  if (len > 0) {
    memcpy(body + 2, payload, len);
  }
  const uint8_t bodyLen = static_cast<uint8_t>(len + 2);
  put16(body + bodyLen, crc16(body, bodyLen));

  uint8_t encoded[kMaxEncoded];
  size_t encodedLen = cobsEncode(body, bodyLen + 2, encoded);
  encoded[encodedLen++] = 0;

  if (droppable && out_.availableForWrite() < static_cast<int>(encodedLen)) {
    if (dropped_ != 0xFFFF) {
      ++dropped_;
    }
    return false;
  }
  // The sequence number only advances for frames that are actually sent, so
  // the host can count frames lost on the wire separately from frames dropped
  // here (reported in Status).
  ++seq_;
  out_.write(encoded, encodedLen);
  return true;
}

bool FrameReader::feed(uint8_t byte) {
  if (byte != 0) {
    if (len_ < sizeof(buf_)) {
      buf_[len_++] = byte;
    } else {
      overflow_ = true;
    }
    return false;
  }

  // Delimiter: try to decode whatever was accumulated.
  const bool overflow = overflow_;
  const uint8_t rawLen = len_;
  len_ = 0;
  overflow_ = false;
  if (rawLen == 0) {
    return false;  // empty frame, e.g. a resynchronisation delimiter
  }
  if (overflow) {
    ++errors_;
    return false;
  }
  const size_t decoded = cobsDecodeInPlace(buf_, rawLen);
  if (decoded < 4 || decoded > kMaxBody) {
    ++errors_;
    return false;
  }
  const uint16_t expected = get16(buf_ + decoded - 2);
  if (crc16(buf_, decoded - 2) != expected) {
    ++errors_;
    return false;
  }
  payloadLen_ = static_cast<uint8_t>(decoded - 4);
  return true;
}

}  // namespace proto
}  // namespace tb
