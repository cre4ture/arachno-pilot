#![no_std]

use core::fmt;

pub const SYNC_0: u8 = 0xA5;
pub const SYNC_1: u8 = 0x5A;
pub const PROTOCOL_VERSION: u8 = 1;
pub const HEADER_LEN: usize = 6;
pub const CRC_LEN: usize = 2;
pub const IMU_SAMPLE_PAYLOAD_LEN: usize = 26;
pub const MAX_FRAME_LEN: usize = HEADER_LEN + IMU_SAMPLE_PAYLOAD_LEN + CRC_LEN;

#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub enum FrameKind {
    ImuSample = 0x01,
}

impl TryFrom<u8> for FrameKind {
    type Error = DecodeError;

    fn try_from(value: u8) -> Result<Self, Self::Error> {
        match value {
            0x01 => Ok(Self::ImuSample),
            _ => Err(DecodeError::UnknownFrameKind(value)),
        }
    }
}

#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct ImuSample {
    pub timestamp_us: u32,
    pub accel_mg: [i16; 3],
    pub gyro_mdps: [i32; 3],
    pub temperature_centi_c: i16,
    pub status: u16,
}

#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Frame {
    ImuSample { sequence: u8, sample: ImuSample },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EncodeError {
    OutputTooSmall,
}

impl fmt::Display for EncodeError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::OutputTooSmall => f.write_str("output buffer too small"),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DecodeError {
    IncompleteFrame,
    InvalidSync,
    InvalidVersion(u8),
    UnknownFrameKind(u8),
    InvalidLength { expected: usize, actual: usize },
    PayloadLengthMismatch { kind: FrameKind, length: usize },
    CrcMismatch { expected: u16, actual: u16 },
    FrameTooLong(usize),
}

impl fmt::Display for DecodeError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::IncompleteFrame => f.write_str("incomplete frame"),
            Self::InvalidSync => f.write_str("invalid sync bytes"),
            Self::InvalidVersion(version) => write!(f, "unsupported protocol version {version}"),
            Self::UnknownFrameKind(kind) => write!(f, "unknown frame kind 0x{kind:02x}"),
            Self::InvalidLength { expected, actual } => {
                write!(f, "invalid frame length: expected {expected}, got {actual}")
            }
            Self::PayloadLengthMismatch { kind, length } => {
                write!(f, "invalid payload length {length} for {kind:?}")
            }
            Self::CrcMismatch { expected, actual } => {
                write!(
                    f,
                    "crc mismatch: expected 0x{expected:04x}, got 0x{actual:04x}"
                )
            }
            Self::FrameTooLong(length) => write!(f, "frame too long: {length} bytes"),
        }
    }
}

#[derive(Debug, Clone)]
pub struct FrameParser {
    buffer: [u8; MAX_FRAME_LEN],
    len: usize,
    expected_len: usize,
}

impl Default for FrameParser {
    fn default() -> Self {
        Self::new()
    }
}

impl FrameParser {
    pub const fn new() -> Self {
        Self {
            buffer: [0; MAX_FRAME_LEN],
            len: 0,
            expected_len: 0,
        }
    }

    pub fn reset(&mut self) {
        self.len = 0;
        self.expected_len = 0;
    }

    pub fn push(&mut self, byte: u8) -> Result<Option<Frame>, DecodeError> {
        if self.len == 0 {
            if byte == SYNC_0 {
                self.buffer[0] = byte;
                self.len = 1;
            }
            return Ok(None);
        }

        if self.len == 1 {
            if byte == SYNC_1 {
                self.buffer[1] = byte;
                self.len = 2;
            } else if byte == SYNC_0 {
                self.buffer[0] = byte;
                self.len = 1;
            } else {
                self.reset();
            }
            return Ok(None);
        }

        if self.len >= MAX_FRAME_LEN {
            self.reset();
            return Err(DecodeError::FrameTooLong(self.len + 1));
        }

        self.buffer[self.len] = byte;
        self.len += 1;

        if self.len == HEADER_LEN {
            if self.buffer[2] != PROTOCOL_VERSION {
                let version = self.buffer[2];
                self.reset();
                return Err(DecodeError::InvalidVersion(version));
            }

            let kind = FrameKind::try_from(self.buffer[3])?;
            let payload_len = self.buffer[4] as usize;
            let expected_len = frame_len(payload_len);

            if expected_len > MAX_FRAME_LEN {
                self.reset();
                return Err(DecodeError::FrameTooLong(expected_len));
            }

            let required_payload_len = expected_payload_len(kind);
            if payload_len != required_payload_len {
                self.reset();
                return Err(DecodeError::PayloadLengthMismatch {
                    kind,
                    length: payload_len,
                });
            }

            self.expected_len = expected_len;
        }

        if self.expected_len != 0 && self.len == self.expected_len {
            let parsed = decode_frame(&self.buffer[..self.expected_len]);
            self.reset();
            return parsed.map(Some);
        }

        Ok(None)
    }

    pub fn push_slice(&mut self, bytes: &[u8]) -> Result<Option<Frame>, DecodeError> {
        for &byte in bytes {
            if let Some(frame) = self.push(byte)? {
                return Ok(Some(frame));
            }
        }

        Ok(None)
    }
}

pub fn encode_sample_frame(
    sequence: u8,
    sample: &ImuSample,
    out: &mut [u8],
) -> Result<usize, EncodeError> {
    let frame_len = MAX_FRAME_LEN;
    if out.len() < frame_len {
        return Err(EncodeError::OutputTooSmall);
    }

    out[0] = SYNC_0;
    out[1] = SYNC_1;
    out[2] = PROTOCOL_VERSION;
    out[3] = FrameKind::ImuSample as u8;
    out[4] = IMU_SAMPLE_PAYLOAD_LEN as u8;
    out[5] = sequence;

    let mut cursor = HEADER_LEN;
    cursor = write_u32(out, cursor, sample.timestamp_us);

    for value in sample.accel_mg {
        cursor = write_i16(out, cursor, value);
    }

    for value in sample.gyro_mdps {
        cursor = write_i32(out, cursor, value);
    }

    cursor = write_i16(out, cursor, sample.temperature_centi_c);
    cursor = write_u16(out, cursor, sample.status);

    let crc = crc16_ccitt(&out[..cursor]);
    let [crc_low, crc_high] = crc.to_le_bytes();
    out[cursor] = crc_low;
    out[cursor + 1] = crc_high;

    Ok(frame_len)
}

pub fn decode_frame(bytes: &[u8]) -> Result<Frame, DecodeError> {
    if bytes.len() < HEADER_LEN + CRC_LEN {
        return Err(DecodeError::IncompleteFrame);
    }

    if bytes[0] != SYNC_0 || bytes[1] != SYNC_1 {
        return Err(DecodeError::InvalidSync);
    }

    if bytes[2] != PROTOCOL_VERSION {
        return Err(DecodeError::InvalidVersion(bytes[2]));
    }

    let kind = FrameKind::try_from(bytes[3])?;
    let payload_len = bytes[4] as usize;
    let expected_len = frame_len(payload_len);

    if bytes.len() != expected_len {
        return Err(DecodeError::InvalidLength {
            expected: expected_len,
            actual: bytes.len(),
        });
    }

    let actual_crc = u16::from_le_bytes([bytes[bytes.len() - 2], bytes[bytes.len() - 1]]);
    let expected_crc = crc16_ccitt(&bytes[..bytes.len() - CRC_LEN]);
    if actual_crc != expected_crc {
        return Err(DecodeError::CrcMismatch {
            expected: expected_crc,
            actual: actual_crc,
        });
    }

    match kind {
        FrameKind::ImuSample => {
            if payload_len != IMU_SAMPLE_PAYLOAD_LEN {
                return Err(DecodeError::PayloadLengthMismatch {
                    kind,
                    length: payload_len,
                });
            }

            let sequence = bytes[5];
            let payload = &bytes[HEADER_LEN..bytes.len() - CRC_LEN];
            let mut cursor = 0usize;

            let timestamp_us = read_u32(payload, &mut cursor);
            let mut accel_mg = [0i16; 3];
            for value in &mut accel_mg {
                *value = read_i16(payload, &mut cursor);
            }

            let mut gyro_mdps = [0i32; 3];
            for value in &mut gyro_mdps {
                *value = read_i32(payload, &mut cursor);
            }

            let temperature_centi_c = read_i16(payload, &mut cursor);
            let status = read_u16(payload, &mut cursor);

            Ok(Frame::ImuSample {
                sequence,
                sample: ImuSample {
                    timestamp_us,
                    accel_mg,
                    gyro_mdps,
                    temperature_centi_c,
                    status,
                },
            })
        }
    }
}

pub fn crc16_ccitt(bytes: &[u8]) -> u16 {
    let mut crc = 0xFFFFu16;

    for &byte in bytes {
        crc ^= (byte as u16) << 8;
        for _ in 0..8 {
            if (crc & 0x8000) != 0 {
                crc = (crc << 1) ^ 0x1021;
            } else {
                crc <<= 1;
            }
        }
    }

    crc
}

const fn frame_len(payload_len: usize) -> usize {
    HEADER_LEN + payload_len + CRC_LEN
}

const fn expected_payload_len(kind: FrameKind) -> usize {
    match kind {
        FrameKind::ImuSample => IMU_SAMPLE_PAYLOAD_LEN,
    }
}

fn write_u16(out: &mut [u8], cursor: usize, value: u16) -> usize {
    let bytes = value.to_le_bytes();
    out[cursor..cursor + 2].copy_from_slice(&bytes);
    cursor + 2
}

fn write_i16(out: &mut [u8], cursor: usize, value: i16) -> usize {
    let bytes = value.to_le_bytes();
    out[cursor..cursor + 2].copy_from_slice(&bytes);
    cursor + 2
}

fn write_u32(out: &mut [u8], cursor: usize, value: u32) -> usize {
    let bytes = value.to_le_bytes();
    out[cursor..cursor + 4].copy_from_slice(&bytes);
    cursor + 4
}

fn write_i32(out: &mut [u8], cursor: usize, value: i32) -> usize {
    let bytes = value.to_le_bytes();
    out[cursor..cursor + 4].copy_from_slice(&bytes);
    cursor + 4
}

fn read_u16(bytes: &[u8], cursor: &mut usize) -> u16 {
    let value = u16::from_le_bytes([bytes[*cursor], bytes[*cursor + 1]]);
    *cursor += 2;
    value
}

fn read_i16(bytes: &[u8], cursor: &mut usize) -> i16 {
    let value = i16::from_le_bytes([bytes[*cursor], bytes[*cursor + 1]]);
    *cursor += 2;
    value
}

fn read_u32(bytes: &[u8], cursor: &mut usize) -> u32 {
    let value = u32::from_le_bytes([
        bytes[*cursor],
        bytes[*cursor + 1],
        bytes[*cursor + 2],
        bytes[*cursor + 3],
    ]);
    *cursor += 4;
    value
}

fn read_i32(bytes: &[u8], cursor: &mut usize) -> i32 {
    let value = i32::from_le_bytes([
        bytes[*cursor],
        bytes[*cursor + 1],
        bytes[*cursor + 2],
        bytes[*cursor + 3],
    ]);
    *cursor += 4;
    value
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sample_frame_roundtrips() {
        let sample = ImuSample {
            timestamp_us: 42_000,
            accel_mg: [12, -34, 56],
            gyro_mdps: [123_456, -234_567, 345_678],
            temperature_centi_c: 2_750,
            status: 0x0011,
        };
        let mut buf = [0u8; MAX_FRAME_LEN];
        let written = encode_sample_frame(7, &sample, &mut buf).unwrap();
        let frame = decode_frame(&buf[..written]).unwrap();

        assert_eq!(
            frame,
            Frame::ImuSample {
                sequence: 7,
                sample,
            }
        );
    }

    #[test]
    fn parser_skips_leading_noise() {
        let sample = ImuSample {
            timestamp_us: 1,
            accel_mg: [1, 2, 3],
            gyro_mdps: [4, 5, 6],
            temperature_centi_c: 7,
            status: 8,
        };
        let mut frame_buf = [0u8; MAX_FRAME_LEN];
        encode_sample_frame(9, &sample, &mut frame_buf).unwrap();

        let mut parser = FrameParser::new();
        assert!(parser.push_slice(&[0x00, 0xFF, 0xA5]).unwrap().is_none());

        let frame = parser.push_slice(&frame_buf).unwrap().unwrap();
        assert_eq!(
            frame,
            Frame::ImuSample {
                sequence: 9,
                sample,
            }
        );
    }

    #[test]
    fn crc_mismatch_is_detected() {
        let sample = ImuSample::default();
        let mut buf = [0u8; MAX_FRAME_LEN];
        let written = encode_sample_frame(0, &sample, &mut buf).unwrap();
        buf[written - 1] ^= 0xFF;

        let err = decode_frame(&buf[..written]).unwrap_err();
        assert!(matches!(err, DecodeError::CrcMismatch { .. }));
    }
}
