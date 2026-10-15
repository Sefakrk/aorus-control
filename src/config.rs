use std::fs;
use std::path::PathBuf;
use serde::{Deserialize, Serialize};
use crate::hal::models::{FanMode, RgbColor};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AppConfig {
    pub fan_mode: FanMode,
    pub manual_fan_speed: u8,
    pub kb_color: RgbColor,
    pub kb_brightness: u8,
    pub bar_color: RgbColor,
    pub bar_enabled: bool,
    pub battery_limit: bool,
}

impl Default for AppConfig {
    fn default() -> Self {
        Self {
            fan_mode: FanMode::Normal,
            manual_fan_speed: 60,
            kb_color: RgbColor::Blue,
            kb_brightness: 100,
            bar_color: RgbColor::Blue,
            bar_enabled: true,
            battery_limit: true,
        }
    }
}

impl AppConfig {
    fn get_path() -> PathBuf {
        if let Ok(exe) = std::env::current_exe() {
            if let Some(dir) = exe.parent() {
                return dir.join("aorus_config.json");
            }
        }
        PathBuf::from("aorus_config.json")
    }

    pub fn load() -> Self {
        let path = Self::get_path();
        if path.exists() {
            if let Ok(content) = fs::read_to_string(&path) {
                if let Ok(cfg) = serde_json::from_str::<AppConfig>(&content) {
                    return cfg;
                }
            }
        }
        let default = Self::default();
        let _ = default.save();
        default
    }

    pub fn save(&self) -> Result<(), String> {
        let path = Self::get_path();
        let json = serde_json::to_string_pretty(self).map_err(|e| e.to_string())?;
        fs::write(path, json).map_err(|e| e.to_string())?;
        Ok(())
    }
}
