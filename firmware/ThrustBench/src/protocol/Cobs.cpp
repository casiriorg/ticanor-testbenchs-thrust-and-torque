#include "Cobs.h"

namespace tb {
namespace proto {

size_t cobsEncode(const uint8_t* in, size_t len, uint8_t* out) {
  size_t codeIndex = 0;  // position of the current block's length byte
  size_t write = 1;
  uint8_t code = 1;
  for (size_t i = 0; i < len; ++i) {
    if (in[i] == 0) {
      out[codeIndex] = code;
      codeIndex = write++;
      code = 1;
    } else {
      out[write++] = in[i];
      ++code;
      // Bodies are limited to 253 bytes, so a block can never reach 0xFF.
    }
  }
  out[codeIndex] = code;
  return write;
}

size_t cobsDecodeInPlace(uint8_t* buf, size_t len) {
  size_t read = 0;
  size_t write = 0;
  while (read < len) {
    const uint8_t code = buf[read++];
    if (code == 0 || read + code - 1 > len) {
      return 0;
    }
    for (uint8_t i = 1; i < code; ++i) {
      buf[write++] = buf[read++];
    }
    if (read < len) {
      buf[write++] = 0;
    }
  }
  return write;
}

}  // namespace proto
}  // namespace tb
