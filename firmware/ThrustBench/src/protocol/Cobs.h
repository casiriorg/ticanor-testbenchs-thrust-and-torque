// Consistent Overhead Byte Stuffing. Removes every 0x00 from a frame body so
// 0x00 can be used as an unambiguous frame delimiter on the serial line.
#pragma once

#include <stddef.h>
#include <stdint.h>

namespace tb {
namespace proto {

// Encodes `len` bytes (len <= 253). `out` must hold len + 1 bytes.
// Returns the encoded length (without the trailing delimiter).
size_t cobsEncode(const uint8_t* in, size_t len, uint8_t* out);

// Decodes in place. Returns the decoded length, or 0 if the input is malformed.
size_t cobsDecodeInPlace(uint8_t* buf, size_t len);

}  // namespace proto
}  // namespace tb
