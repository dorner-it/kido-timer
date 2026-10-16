use serde::Serialize;

use crate::protocol::{ChannelStatus, DeviceMode, StateFlag, TimeChannel, TimerFrame, NUM_CHANNELS};

#[derive(Serialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct ChannelDto {
    pub time_ms: u16,
    pub status: &'static str,
    pub status_raw: u8,
}

impl From<TimeChannel> for ChannelDto {
    fn from(c: TimeChannel) -> Self {
        let (status, status_raw) = match c.status {
            ChannelStatus::Inactive => ("inactive", 0x00),
            ChannelStatus::Running => ("running", 0x81),
            ChannelStatus::Captured => ("captured", 0x82),
            ChannelStatus::Confirmed => ("confirmed", 0x84),
            ChannelStatus::Unknown(v) => ("unknown", v),
        };
        Self {
            time_ms: c.time_ms,
            status,
            status_raw,
        }
    }
}

#[derive(Serialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct FrameDto {
    pub device_mode: &'static str,
    pub device_mode_raw: u8,
    pub lane: u8,
    pub state_flag: &'static str,
    pub state_flag_raw: u8,
    pub channels: [ChannelDto; NUM_CHANNELS],
    pub raw_hex: String,
}

impl From<&TimerFrame> for FrameDto {
    fn from(f: &TimerFrame) -> Self {
        let (device_mode, device_mode_raw) = match f.device_mode {
            DeviceMode::Standby => ("standby", 0x00),
            DeviceMode::Active => ("active", 0x02),
            DeviceMode::Unknown(v) => ("unknown", v),
        };
        let (state_flag, state_flag_raw) = match f.state_flag {
            StateFlag::Measuring => ("measuring", 0x00),
            StateFlag::Armed => ("armed", 0x01),
            StateFlag::Unknown(v) => ("unknown", v),
        };
        let channels = std::array::from_fn(|i| ChannelDto::from(f.channels[i]));
        Self {
            device_mode,
            device_mode_raw,
            lane: f.lane,
            state_flag,
            state_flag_raw,
            channels,
            raw_hex: f.raw_hex_string(),
        }
    }
}

#[derive(Serialize, Clone)]
#[serde(rename_all = "camelCase", tag = "type")]
pub enum ConnectionEvent {
    Frame { frame: FrameDto },
    Error { message: String },
    Disconnected,
    Connected { source: String },
}

