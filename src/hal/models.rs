use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum FanMode {
    Quiet,
    Normal,
    Gaming,
    Turbo,
    Manual(u8),
}

impl Default for FanMode {
    fn default() -> Self {
        Self::Normal
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[repr(u8)]
pub enum RgbColor {
    Off = 0,
    Red = 1,
    Green = 2,
    Yellow = 3,
    Blue = 4,
    Orange = 5,
    Purple = 6,
    White = 7,
}

impl Default for RgbColor {
    fn default() -> Self {
        Self::Blue
    }
}

impl RgbColor {
    #[allow(dead_code)]
    pub fn from_u8(val: u8) -> Self {
        match val {
            1 => Self::Red,
            2 => Self::Green,
            3 => Self::Yellow,
            4 => Self::Blue,
            5 => Self::Orange,
            6 => Self::Purple,
            7 => Self::White,
            _ => Self::Off,
        }
    }

    pub fn to_u8(self) -> u8 {
        self as u8
    }

    pub fn from_name(name: &str) -> Self {
        match name {
            "Red" => Self::Red,
            "Green" => Self::Green,
            "Yellow" => Self::Yellow,
            "Blue" => Self::Blue,
            "Orange" => Self::Orange,
            "Purple" => Self::Purple,
            "White" => Self::White,
            _ => Self::Off,
        }
    }

    pub fn to_name(self) -> &'static str {
        match self {
            Self::Red => "Red",
            Self::Green => "Green",
            Self::Yellow => "Yellow",
            Self::Blue => "Blue",
            Self::Orange => "Orange",
            Self::Purple => "Purple",
            Self::White => "White",
            Self::Off => "Off",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum RgbZone {
    Keyboard,
    RearBar,
}

#[derive(Debug, Clone, Copy, Default)]
pub struct TelemetryData {
    pub cpu_temp: u8,
    pub gpu_temp: u8,
    pub fan1_rpm: u16,
    pub fan2_rpm: u16,
}
