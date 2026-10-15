pub mod models;
pub mod windows;

pub use models::*;

pub trait ThermalController: Send + Sync {
    fn set_fan_mode(&mut self, mode: FanMode) -> Result<(), String>;
    fn set_custom_fan_speed(&mut self, speed_percent: u8) -> Result<(), String>;
    #[allow(dead_code)]
    fn get_telemetry(&self) -> Result<TelemetryData, String>;
    fn set_battery_charge_limit(&mut self, enabled: bool) -> Result<(), String>;
    fn get_gpu_mode(&self) -> Result<GpuMode, String>;
    fn set_gpu_mode(&mut self, mode: GpuMode) -> Result<(), String>;
}

pub trait LightingController: Send + Sync {
    fn set_zone_lighting(&mut self, zone: RgbZone, color: RgbColor, brightness: u8) -> Result<(), String>;
}

pub fn create_controllers() -> (Box<dyn ThermalController>, Box<dyn LightingController>) {
    (
        Box::new(windows::WindowsThermalController::new()),
        Box::new(windows::WindowsLightingController::new()),
    )
}
