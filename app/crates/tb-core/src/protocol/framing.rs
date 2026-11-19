//! COBS framing with a CRC-16/CCITT-FALSE trailer.
//!
//! Frame body: `type(1) seq(1) payload(n) crc16(2, little endian)`.
//! The body is COBS encoded and terminated by one `0x00` byte.

use super::ids::{MAX_BODY, MAX_PAYLOAD};
use crate::error::{Error, Result};

/// CRC-16/CCITT-FALSE: poly 0x1021, init 0xFFFF.
pub fn crc16(data: &[u8]) -> u16 {
    let mut crc: u16 = 0xFFFF;
    for &byte in data {
        crc ^= (byte as u16) << 8;
        for _ in 0..8 {
            crc = if crc & 0x8000 != 0 { (crc << 1) ^ 0x1021 } else { crc << 1 };
        }
    }
    crc
}

pub fn cobs_encode(input: &[u8]) -> Vec<u8> {
    let mut out = Vec::with_capacity(input.len() + 2);
    let mut code_index = 0;
    out.push(0);
    let mut code: u8 = 1;
    for &byte in input {
        if byte == 0 {
            out[code_index] = code;
            code_index = out.len();
            out.push(0);
            code = 1;
        } else {
            out.push(byte);
            code += 1;
            if code == 0xFF {
                out[code_index] = code;
                code_index = out.len();
                out.push(0);
                code = 1;
            }
        }
    }
    out[code_index] = code;
    out
}

pub fn cobs_decode(input: &[u8]) -> Option<Vec<u8>> {
    let mut out = Vec::with_capacity(input.len());
    let mut read = 0;
    while read < input.len() {
        let code = input[read] as usize;
        read += 1;
        if code == 0 || read + code - 1 > input.len() {
            return None;
        }
        out.extend_from_slice(&input[read..read + code - 1]);
        read += code - 1;
        if code != 0xFF && read < input.len() {
            out.push(0);
        }
    }
    Some(out)
}

/// A decoded frame, CRC already verified.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Frame {
    pub kind: u8,
    pub seq: u8,
    pub payload: Vec<u8>,
}

/// Builds the bytes to put on the wire, including the trailing delimiter.
pub fn encode_frame(kind: u8, seq: u8, payload: &[u8]) -> Result<Vec<u8>> {
    if payload.len() > MAX_PAYLOAD {
        return Err(Error::Protocol(format!("payload of {} bytes is too long", payload.len())));
    }
    let mut body = Vec::with_capacity(payload.len() + 4);
    body.push(kind);
    body.push(seq);
    body.extend_from_slice(payload);
    let crc = crc16(&body);
    body.extend_from_slice(&crc.to_le_bytes());
    let mut wire = cobs_encode(&body);
    wire.push(0);
    Ok(wire)
}

/// Incremental decoder: feed it whatever the port returned, get whole frames.
#[derive(Default)]
pub struct FrameDecoder {
    buffer: Vec<u8>,
    overflow: bool,
    errors: u64,
}

impl FrameDecoder {
    pub fn new() -> Self {
        Self::default()
    }

    /// Frames that failed COBS, length or CRC validation since creation.
    pub fn errors(&self) -> u64 {
        self.errors
    }

    pub fn push(&mut self, bytes: &[u8], out: &mut Vec<Frame>) {
        for &byte in bytes {
            if byte != 0 {
                if self.buffer.len() < MAX_BODY + 2 {
                    self.buffer.push(byte);
                } else {
                    self.overflow = true;
                }
                continue;
            }
            let raw = std::mem::take(&mut self.buffer);
            let overflow = std::mem::replace(&mut self.overflow, false);
            if raw.is_empty() {
                continue;
            }
            if overflow {
                self.errors += 1;
                continue;
            }
            match Self::decode_body(&raw) {
                Some(frame) => out.push(frame),
                None => self.errors += 1,
            }
        }
    }

    fn decode_body(raw: &[u8]) -> Option<Frame> {
        let body = cobs_decode(raw)?;
        if body.len() < 4 || body.len() > MAX_BODY {
            return None;
        }
        let (content, crc_bytes) = body.split_at(body.len() - 2);
        let expected = u16::from_le_bytes([crc_bytes[0], crc_bytes[1]]);
        if crc16(content) != expected {
            return None;
        }
        Some(Frame { kind: content[0], seq: content[1], payload: content[2..].to_vec() })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn crc_matches_reference_vector() {
        // Standard check value for CRC-16/CCITT-FALSE.
        assert_eq!(crc16(b"123456789"), 0x29B1);
    }

    #[test]
    fn cobs_round_trips_including_zeros() {
        for data in [&[][..], &[0], &[1, 0, 2], &[0, 0, 0], &[5, 4, 3, 2, 1]] {
            let encoded = cobs_encode(data);
            assert!(!encoded.contains(&0));
            assert_eq!(cobs_decode(&encoded).unwrap(), data);
        }
    }

    #[test]
    fn frames_round_trip_and_survive_split_reads() {
        let wire = encode_frame(0x83, 7, &[0, 1, 0, 2, 0]).unwrap();
        let mut decoder = FrameDecoder::new();
        let mut frames = Vec::new();
        let (a, b) = wire.split_at(3);
        decoder.push(a, &mut frames);
        assert!(frames.is_empty());
        decoder.push(b, &mut frames);
        assert_eq!(frames, vec![Frame { kind: 0x83, seq: 7, payload: vec![0, 1, 0, 2, 0] }]);
    }

    #[test]
    fn corrupted_frame_is_counted_and_next_frame_still_decodes() {
        let mut wire = encode_frame(0x86, 1, &[1, 2, 3]).unwrap();
        wire[2] ^= 0x55;
        wire.extend(encode_frame(0x86, 2, &[4]).unwrap());
        let mut decoder = FrameDecoder::new();
        let mut frames = Vec::new();
        decoder.push(&wire, &mut frames);
        assert_eq!(decoder.errors(), 1);
        assert_eq!(frames.len(), 1);
        assert_eq!(frames[0].seq, 2);
    }
}
