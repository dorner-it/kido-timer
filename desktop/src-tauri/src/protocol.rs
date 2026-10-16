/// KiDo-Timer serial protocol parser.
///
/// The device sends fixed 41-byte frames at 9600 baud (8N1).
///
/// Frame layout (41 bytes):
/// ```text
/// Offset  Size  Description
/// ------  ----  -----------
///  0..2     3   Header: 0x52 0x57 0x3A ("RW:")
///  3        1   Protocol version (observed: 0x01)
///  4        1   Device mode: 0x00=Standby, 0x02=Active
///  5        1   Lane number (1-based)
///  6        1   State flag: 0x00=Measuring, 0x01=Armed
///  7..10    4   Channel 1: [time_lo, time_hi, reserved, status]
/// 11..14    4   Channel 2: [time_lo, time_hi, reserved, status]
/// 15..18    4   Channel 3: [time_lo, time_hi, reserved, status]
/// 19..22    4   Channel 4: [time_lo, time_hi, reserved, status]
/// 23..38   16   Reserved (all zeros in observed data)
/// 39        1   Terminator: 0x0D (CR)
/// 40        1   Trailing null: 0x00
/// ```
///
/// Channel status bytes:
///   0x00 = Inactive (no sensor / not triggered)
///   0x81 = Running (timer counting)
///   0x82 = Captured (time frozen, pending confirmation)
///   0x84 = Confirmed (final time)
///
/// Time is stored as little-endian u16 milliseconds (max 65535ms ≈ 65.5s).

use std::fmt;

// -- Constants --

pub const FRAME_SIZE: usize = 41;
pub const HEADER: [u8; 3] = [0x52, 0x57, 0x3A]; // "RW:"
pub const TERMINATOR: u8 = 0x0D;
#[allow(dead_code)]
pub const TRAILING_NULL: u8 = 0x00;
pub const NUM_CHANNELS: usize = 4;

// Byte offsets
pub const OFF_HEADER: usize = 0;
pub const OFF_VERSION: usize = 3;
pub const OFF_DEVICE_MODE: usize = 4;
pub const OFF_LANE: usize = 5;
pub const OFF_STATE_FLAG: usize = 6;
pub const OFF_CHANNELS: usize = 7; // 4 channels × 4 bytes each
#[allow(dead_code)]
pub const OFF_RESERVED: usize = 23;
pub const OFF_TERMINATOR: usize = 39;
#[allow(dead_code)]
pub const OFF_TRAILING: usize = 40;

pub const CHANNEL_SIZE: usize = 4;

// -- Enums --

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DeviceMode {
    Standby,
    Active,
    Unknown(u8),
}

impl From<u8> for DeviceMode {
    fn from(b: u8) -> Self {
        match b {
            0x00 => DeviceMode::Standby,
            0x02 => DeviceMode::Active,
            other => DeviceMode::Unknown(other),
        }
    }
}

impl fmt::Display for DeviceMode {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            DeviceMode::Standby => write!(f, "STANDBY"),
            DeviceMode::Active => write!(f, "ACTIVE"),
            DeviceMode::Unknown(v) => write!(f, "UNKNOWN(0x{v:02X})"),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StateFlag {
    Measuring,
    Armed,
    Unknown(u8),
}

impl From<u8> for StateFlag {
    fn from(b: u8) -> Self {
        match b {
            0x00 => StateFlag::Measuring,
            0x01 => StateFlag::Armed,
            other => StateFlag::Unknown(other),
        }
    }
}

impl fmt::Display for StateFlag {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            StateFlag::Measuring => write!(f, "MEASURING"),
            StateFlag::Armed => write!(f, "ARMED"),
            StateFlag::Unknown(v) => write!(f, "UNKNOWN(0x{v:02X})"),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ChannelStatus {
    Inactive,
    Running,
    Captured,
    Confirmed,
    Unknown(u8),
}

impl From<u8> for ChannelStatus {
    fn from(b: u8) -> Self {
        match b {
            0x00 => ChannelStatus::Inactive,
            0x81 => ChannelStatus::Running,
            0x82 => ChannelStatus::Captured,
            0x84 => ChannelStatus::Confirmed,
            other => ChannelStatus::Unknown(other),
        }
    }
}

impl fmt::Display for ChannelStatus {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ChannelStatus::Inactive => write!(f, "INACTIVE"),
            ChannelStatus::Running => write!(f, "RUNNING"),
            ChannelStatus::Captured => write!(f, "CAPTURED"),
            ChannelStatus::Confirmed => write!(f, "CONFIRMED"),
            ChannelStatus::Unknown(v) => write!(f, "UNKNOWN(0x{v:02X})"),
        }
    }
}

// -- Structs --

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TimeChannel {
    pub time_ms: u16,
    pub status: ChannelStatus,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TimerFrame {
    pub version: u8,
    pub device_mode: DeviceMode,
    pub lane: u8,
    pub state_flag: StateFlag,
    pub channels: [TimeChannel; NUM_CHANNELS],
    pub raw_bytes: [u8; FRAME_SIZE],
}

impl TimerFrame {
    /// Returns a hex dump string of the raw frame bytes.
    pub fn raw_hex_string(&self) -> String {
        self.raw_bytes
            .iter()
            .map(|b| format!("{:02X}", b))
            .collect::<Vec<_>>()
            .join(" ")
    }
}

// -- Errors --

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ParseError {
    InvalidHeader,
    InvalidTerminator { found: u8 },
    UnsupportedVersion { version: u8 },
}

impl fmt::Display for ParseError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ParseError::InvalidHeader => write!(f, "invalid header (expected \"RW:\")"),
            ParseError::InvalidTerminator { found } => {
                write!(f, "invalid terminator: 0x{found:02X} (expected 0x0D)")
            }
            ParseError::UnsupportedVersion { version } => {
                write!(f, "unsupported version: {version}")
            }
        }
    }
}

impl std::error::Error for ParseError {}

// -- Parser --

/// Parse a 41-byte frame into a `TimerFrame`.
pub fn parse_frame(data: &[u8; FRAME_SIZE]) -> Result<TimerFrame, ParseError> {
    // Validate header
    if data[OFF_HEADER..OFF_HEADER + 3] != HEADER {
        return Err(ParseError::InvalidHeader);
    }

    // Validate terminator
    if data[OFF_TERMINATOR] != TERMINATOR {
        return Err(ParseError::InvalidTerminator {
            found: data[OFF_TERMINATOR],
        });
    }

    // Check version (only version 1 observed)
    let version = data[OFF_VERSION];
    if version == 0 {
        return Err(ParseError::UnsupportedVersion { version });
    }

    // Parse fields
    let device_mode = DeviceMode::from(data[OFF_DEVICE_MODE]);
    let lane = data[OFF_LANE];
    let state_flag = StateFlag::from(data[OFF_STATE_FLAG]);

    // Parse channels
    let mut channels = [TimeChannel {
        time_ms: 0,
        status: ChannelStatus::Inactive,
    }; NUM_CHANNELS];

    for i in 0..NUM_CHANNELS {
        let base = OFF_CHANNELS + i * CHANNEL_SIZE;
        let time_lo = data[base] as u16;
        let time_hi = data[base + 1] as u16;
        // data[base + 2] is reserved (always 0x00)
        let status_byte = data[base + 3];

        channels[i] = TimeChannel {
            time_ms: time_lo | (time_hi << 8),
            status: ChannelStatus::from(status_byte),
        };
    }

    Ok(TimerFrame {
        version,
        device_mode,
        lane,
        state_flag,
        channels,
        raw_bytes: *data,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Build a frame from components for testing.
    fn make_frame(
        version: u8,
        mode: u8,
        lane: u8,
        state: u8,
        ch1: [u8; 4],
        ch2: [u8; 4],
        ch3: [u8; 4],
        ch4: [u8; 4],
    ) -> [u8; FRAME_SIZE] {
        let mut frame = [0u8; FRAME_SIZE];
        frame[0] = 0x52;
        frame[1] = 0x57;
        frame[2] = 0x3A;
        frame[3] = version;
        frame[4] = mode;
        frame[5] = lane;
        frame[6] = state;
        frame[7..11].copy_from_slice(&ch1);
        frame[11..15].copy_from_slice(&ch2);
        frame[15..19].copy_from_slice(&ch3);
        frame[19..23].copy_from_slice(&ch4);
        frame[39] = 0x0D;
        frame[40] = 0x00;
        frame
    }

    #[test]
    fn test_parse_standby_frame() {
        // Frame from hex dump: all zeros, standby mode
        let frame = make_frame(
            0x01, 0x00, 0x01, 0x00,
            [0, 0, 0, 0],
            [0, 0, 0, 0],
            [0, 0, 0, 0],
            [0, 0, 0, 0],
        );
        let parsed = parse_frame(&frame).unwrap();
        assert_eq!(parsed.version, 1);
        assert_eq!(parsed.device_mode, DeviceMode::Standby);
        assert_eq!(parsed.lane, 1);
        assert_eq!(parsed.state_flag, StateFlag::Measuring);
        for ch in &parsed.channels {
            assert_eq!(ch.status, ChannelStatus::Inactive);
            assert_eq!(ch.time_ms, 0);
        }
    }

    #[test]
    fn test_parse_active_running() {
        // Two channels running at 100ms (0x0064)
        let frame = make_frame(
            0x01, 0x02, 0x01, 0x00,
            [0x64, 0x00, 0x00, 0x81],
            [0x64, 0x00, 0x00, 0x81],
            [0, 0, 0, 0],
            [0, 0, 0, 0],
        );
        let parsed = parse_frame(&frame).unwrap();
        assert_eq!(parsed.device_mode, DeviceMode::Active);
        assert_eq!(parsed.channels[0].time_ms, 100);
        assert_eq!(parsed.channels[0].status, ChannelStatus::Running);
        assert_eq!(parsed.channels[1].time_ms, 100);
        assert_eq!(parsed.channels[1].status, ChannelStatus::Running);
        assert_eq!(parsed.channels[2].status, ChannelStatus::Inactive);
        assert_eq!(parsed.channels[3].status, ChannelStatus::Inactive);
    }

    #[test]
    fn test_parse_captured_and_running() {
        // CH1 captured at 1020ms (0x03FC), CH2 running at 1100ms (0x044C)
        let frame = make_frame(
            0x01, 0x02, 0x01, 0x00,
            [0xFC, 0x03, 0x00, 0x82],
            [0x4C, 0x04, 0x00, 0x81],
            [0, 0, 0, 0],
            [0, 0, 0, 0],
        );
        let parsed = parse_frame(&frame).unwrap();
        assert_eq!(parsed.channels[0].time_ms, 1020);
        assert_eq!(parsed.channels[0].status, ChannelStatus::Captured);
        assert_eq!(parsed.channels[1].time_ms, 1100);
        assert_eq!(parsed.channels[1].status, ChannelStatus::Running);
    }

    #[test]
    fn test_parse_confirmed() {
        // CH1=3620ms (0x0E24), CH2=35020ms (0x88CC), both confirmed
        let frame = make_frame(
            0x01, 0x02, 0x01, 0x00,
            [0x24, 0x0E, 0x00, 0x84],
            [0xCC, 0x88, 0x00, 0x84],
            [0, 0, 0, 0],
            [0, 0, 0, 0],
        );
        let parsed = parse_frame(&frame).unwrap();
        assert_eq!(parsed.channels[0].time_ms, 3620);
        assert_eq!(parsed.channels[0].status, ChannelStatus::Confirmed);
        assert_eq!(parsed.channels[1].time_ms, 35020);
        assert_eq!(parsed.channels[1].status, ChannelStatus::Confirmed);
    }

    #[test]
    fn test_parse_armed_state() {
        let frame = make_frame(
            0x01, 0x02, 0x01, 0x01,
            [0, 0, 0, 0],
            [0, 0, 0, 0],
            [0, 0, 0, 0],
            [0, 0, 0, 0],
        );
        let parsed = parse_frame(&frame).unwrap();
        assert_eq!(parsed.state_flag, StateFlag::Armed);
    }

    #[test]
    fn test_invalid_header() {
        let mut frame = [0u8; FRAME_SIZE];
        frame[0] = 0xFF;
        frame[39] = 0x0D;
        let result = parse_frame(&frame);
        assert_eq!(result, Err(ParseError::InvalidHeader));
    }

    #[test]
    fn test_invalid_terminator() {
        let mut frame = make_frame(
            0x01, 0x00, 0x01, 0x00,
            [0, 0, 0, 0], [0, 0, 0, 0], [0, 0, 0, 0], [0, 0, 0, 0],
        );
        frame[39] = 0xFF;
        let result = parse_frame(&frame);
        assert_eq!(result, Err(ParseError::InvalidTerminator { found: 0xFF }));
    }

    #[test]
    fn test_raw_hex_string() {
        let frame = make_frame(
            0x01, 0x02, 0x01, 0x00,
            [0x24, 0x0E, 0x00, 0x84],
            [0xCC, 0x88, 0x00, 0x84],
            [0, 0, 0, 0],
            [0, 0, 0, 0],
        );
        let parsed = parse_frame(&frame).unwrap();
        let hex = parsed.raw_hex_string();
        assert!(hex.starts_with("52 57 3A 01 02 01 00 24 0E 00 84 CC 88 00 84"));
        assert!(hex.ends_with("0D 00"));
    }

    #[test]
    fn test_lane_change() {
        // Lane 2 frame observed in hex dump
        let frame = make_frame(
            0x01, 0x02, 0x02, 0x00,
            [0xFC, 0x03, 0x00, 0x84],
            [0xD6, 0x10, 0x00, 0x84],
            [0, 0, 0, 0],
            [0, 0, 0, 0],
        );
        let parsed = parse_frame(&frame).unwrap();
        assert_eq!(parsed.lane, 2);
        assert_eq!(parsed.channels[0].time_ms, 1020);
        assert_eq!(parsed.channels[1].time_ms, 4310);
    }
}
