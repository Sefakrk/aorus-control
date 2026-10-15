use std::ffi::c_void;
use std::os::windows::process::CommandExt;
use std::process::Command;
use parking_lot::Mutex;
use windows_sys::core::GUID;
use windows_sys::Win32::Devices::DeviceAndDriverInstallation::*;
use windows_sys::Win32::Devices::HumanInterfaceDevice::*;
use windows_sys::Win32::Foundation::*;
use windows_sys::Win32::Graphics::Gdi::*;

use crate::hal::models::*;
use crate::hal::{LightingController, ThermalController};

const CREATE_NO_WINDOW: u32 = 0x08000000;

#[repr(C)]
#[derive(Default, Copy, Clone)]
#[allow(non_snake_case)]
struct FILETIME {
    dwLowDateTime: u32,
    dwHighDateTime: u32,
}

extern "system" {
    fn CreateFileW(
        lpFileName: *const u16,
        dwDesiredAccess: u32,
        dwShareMode: u32,
        lpSecurityAttributes: *const c_void,
        dwCreationDisposition: u32,
        dwFlagsAndAttributes: u32,
        hTemplateFile: *mut c_void,
    ) -> *mut c_void;

    fn GetSystemTimes(
        lpIdleTime: *mut FILETIME,
        lpKernelTime: *mut FILETIME,
        lpUserTime: *mut FILETIME,
    ) -> i32;
}

const GENERIC_READ: u32 = 0x80000000;
const GENERIC_WRITE: u32 = 0x40000000;
const FILE_SHARE_READ: u32 = 1;
const FILE_SHARE_WRITE: u32 = 2;
const OPEN_EXISTING: u32 = 3;

static PREV_CPU_TIMES: Mutex<Option<(u64, u64, u64)>> = Mutex::new(None);

fn filetime_to_u64(ft: &FILETIME) -> u64 {
    ((ft.dwHighDateTime as u64) << 32) | (ft.dwLowDateTime as u64)
}

fn calculate_cpu_usage() -> u8 {
    let mut idle = FILETIME::default();
    let mut kernel = FILETIME::default();
    let mut user = FILETIME::default();

    unsafe {
        if GetSystemTimes(&mut idle, &mut kernel, &mut user) == 0 {
            return 0;
        }
    }

    let i = filetime_to_u64(&idle);
    let k = filetime_to_u64(&kernel);
    let u = filetime_to_u64(&user);

    let mut lock = PREV_CPU_TIMES.lock();
    if let Some((prev_i, prev_k, prev_u)) = *lock {
        let diff_idle = i.saturating_sub(prev_i);
        let diff_kernel = k.saturating_sub(prev_k);
        let diff_user = u.saturating_sub(prev_u);
        let diff_total = diff_kernel + diff_user;

        *lock = Some((i, k, u));

        if diff_total > 0 {
            let busy = diff_total.saturating_sub(diff_idle);
            ((busy as f64 / diff_total as f64) * 100.0).round().clamp(0.0, 100.0) as u8
        } else {
            0
        }
    } else {
        *lock = Some((i, k, u));
        0
    }
}

fn fetch_gpu_usage() -> u8 {
    let output = Command::new("nvidia-smi")
        .args(["--query-gpu=utilization.gpu", "--format=csv,noheader,nounits"])
        .creation_flags(CREATE_NO_WINDOW)
        .output();

    if let Ok(out) = output {
        let text = String::from_utf8_lossy(&out.stdout).trim().to_string();
        text.parse::<u8>().unwrap_or(0)
    } else {
        0
    }
}

// ==========================================
// 1. THERMAL CONTROLLER (WMI ACPI)
// ==========================================

pub struct WindowsThermalController;

impl WindowsThermalController {
    pub fn new() -> Self {
        Self
    }
}

pub fn restore_auto_fans() {
    let script = r#"
        $set = Get-CimInstance -Namespace root/wmi -ClassName GB_WMIACPI_Set -ErrorAction SilentlyContinue | Select-Object -First 1
        if ($set) {
            Invoke-CimMethod -InputObject $set -MethodName SetFixedFanStatus -Arguments @{ Data = [byte]0 } -ErrorAction SilentlyContinue | Out-Null
            Invoke-CimMethod -InputObject $set -MethodName SetStepFanStatus -Arguments @{ Data = [byte]0 } -ErrorAction SilentlyContinue | Out-Null
            Invoke-CimMethod -InputObject $set -MethodName SetAutoFanStatus -Arguments @{ Data = [byte]1 } -ErrorAction SilentlyContinue | Out-Null
            Invoke-CimMethod -InputObject $set -MethodName SetFanModeNotify -Arguments @{ Data = [byte]1 } -ErrorAction SilentlyContinue | Out-Null
        }
    "#;
    run_powershell_silent(script);
}

pub fn apply_pwm_speed(pwm: u8) {
    let script = format!(
        r#"
        $set = Get-CimInstance -Namespace root/wmi -ClassName GB_WMIACPI_Set -ErrorAction SilentlyContinue | Select-Object -First 1
        if ($set) {{
            Invoke-CimMethod -InputObject $set -MethodName SetTurboMode -Arguments @{{ Data = [byte]0 }} -ErrorAction SilentlyContinue | Out-Null
            Invoke-CimMethod -InputObject $set -MethodName SetQuietMode -Arguments @{{ Data = [byte]0 }} -ErrorAction SilentlyContinue | Out-Null
            Invoke-CimMethod -InputObject $set -MethodName SetAutoFanStatus -Arguments @{{ Data = [byte]0 }} -ErrorAction SilentlyContinue | Out-Null
            Invoke-CimMethod -InputObject $set -MethodName SetFixedFanStatus -Arguments @{{ Data = [byte]1 }} -ErrorAction SilentlyContinue | Out-Null
            Invoke-CimMethod -InputObject $set -MethodName SetStepFanStatus -Arguments @{{ Data = [byte]1 }} -ErrorAction SilentlyContinue | Out-Null
            Invoke-CimMethod -InputObject $set -MethodName SetFixedFanSpeed -Arguments @{{ Data = [byte]{} }} -ErrorAction SilentlyContinue | Out-Null
            Invoke-CimMethod -InputObject $set -MethodName SetGpuFanDuty -Arguments @{{ Data = [byte]{} }} -ErrorAction SilentlyContinue | Out-Null
        }}
        "#,
        pwm, pwm
    );
    run_powershell_silent(&script);
}

pub fn fetch_telemetry() -> Result<TelemetryData, String> {
    let script = r#"
        $get = Get-CimInstance -Namespace root/wmi -ClassName GB_WMIACPI_Get -ErrorAction SilentlyContinue | Select-Object -First 1
        if ($get) {
            $cpu = (Invoke-CimMethod -InputObject $get -MethodName getCpuTemp -ErrorAction SilentlyContinue).Data
            $gpu = (Invoke-CimMethod -InputObject $get -MethodName getGpuTemp1 -ErrorAction SilentlyContinue).Data
            $rpm1 = (Invoke-CimMethod -InputObject $get -MethodName getRpm1 -ErrorAction SilentlyContinue).Data
            $rpm2 = (Invoke-CimMethod -InputObject $get -MethodName getRpm2 -ErrorAction SilentlyContinue).Data
            if ($cpu -ne $null -and $rpm1 -ne $null) {
                Write-Output "$cpu,$gpu,$rpm1,$rpm2"
            }
        }
    "#;

    let output = Command::new("powershell")
        .args(["-NoProfile", "-NonInteractive", "-Command", script])
        .creation_flags(CREATE_NO_WINDOW)
        .output()
        .map_err(|e| e.to_string())?;

    let text = String::from_utf8_lossy(&output.stdout).trim().to_string();
    if text.is_empty() {
        return Err("WMI telemetry query returned no data".to_string());
    }

    let parts: Vec<&str> = text.split(',').collect();
    if parts.len() >= 4 {
        let cpu = parts[0].trim().parse::<u8>().map_err(|e| e.to_string())?;
        let gpu = parts[1].trim().parse::<u8>().map_err(|e| e.to_string())?;
        let raw1 = parts[2].trim().parse::<u32>().map_err(|e| e.to_string())?;
        let raw2 = parts[3].trim().parse::<u32>().map_err(|e| e.to_string())?;

        let swap_rpm = |raw: u32| -> u16 {
            if raw == 0 {
                return 0;
            }
            let swapped = (((raw & 0xFF) << 8) | ((raw >> 8) & 0xFF)) as u16;
            if swapped < 8000 {
                swapped
            } else {
                raw as u16
            }
        };

        let cpu_pct = calculate_cpu_usage();
        let gpu_pct = fetch_gpu_usage();

        Ok(TelemetryData {
            cpu_temp: cpu,
            gpu_temp: gpu,
            fan1_rpm: swap_rpm(raw1),
            fan2_rpm: swap_rpm(raw2),
            cpu_usage: cpu_pct,
            gpu_usage: gpu_pct,
        })
    } else {
        Err("Incomplete telemetry data from WMI".to_string())
    }
}

impl ThermalController for WindowsThermalController {
    fn set_fan_mode(&mut self, mode: FanMode) -> Result<(), String> {
        let script = match mode {
            FanMode::Quiet => {
                r#"
                $set = Get-CimInstance -Namespace root/wmi -ClassName GB_WMIACPI_Set -ErrorAction SilentlyContinue | Select-Object -First 1
                if ($set) {
                    Invoke-CimMethod -InputObject $set -MethodName SetFanModeNotify -Arguments @{ Data = [byte]0 } -ErrorAction SilentlyContinue | Out-Null
                    Invoke-CimMethod -InputObject $set -MethodName SetTurboMode -Arguments @{ Data = [byte]0 } -ErrorAction SilentlyContinue | Out-Null
                    Invoke-CimMethod -InputObject $set -MethodName SetQuietMode -Arguments @{ Data = [byte]1 } -ErrorAction SilentlyContinue | Out-Null
                    Invoke-CimMethod -InputObject $set -MethodName SetFixedFanStatus -Arguments @{ Data = [byte]0 } -ErrorAction SilentlyContinue | Out-Null
                    Invoke-CimMethod -InputObject $set -MethodName SetStepFanStatus -Arguments @{ Data = [byte]0 } -ErrorAction SilentlyContinue | Out-Null
                    Invoke-CimMethod -InputObject $set -MethodName SetAutoFanStatus -Arguments @{ Data = [byte]0 } -ErrorAction SilentlyContinue | Out-Null
                    Invoke-CimMethod -InputObject $set -MethodName SetThermalTppStatus -Arguments @{ Data = [byte]0 } -ErrorAction SilentlyContinue | Out-Null
                }
                "#
            }
            FanMode::Normal => {
                r#"
                $set = Get-CimInstance -Namespace root/wmi -ClassName GB_WMIACPI_Set -ErrorAction SilentlyContinue | Select-Object -First 1
                if ($set) {
                    Invoke-CimMethod -InputObject $set -MethodName SetFanModeNotify -Arguments @{ Data = [byte]1 } -ErrorAction SilentlyContinue | Out-Null
                    Invoke-CimMethod -InputObject $set -MethodName SetTurboMode -Arguments @{ Data = [byte]0 } -ErrorAction SilentlyContinue | Out-Null
                    Invoke-CimMethod -InputObject $set -MethodName SetQuietMode -Arguments @{ Data = [byte]0 } -ErrorAction SilentlyContinue | Out-Null
                    Invoke-CimMethod -InputObject $set -MethodName SetFixedFanStatus -Arguments @{ Data = [byte]0 } -ErrorAction SilentlyContinue | Out-Null
                    Invoke-CimMethod -InputObject $set -MethodName SetStepFanStatus -Arguments @{ Data = [byte]0 } -ErrorAction SilentlyContinue | Out-Null
                    Invoke-CimMethod -InputObject $set -MethodName SetAutoFanStatus -Arguments @{ Data = [byte]0 } -ErrorAction SilentlyContinue | Out-Null
                    Invoke-CimMethod -InputObject $set -MethodName SetThermalTppStatus -Arguments @{ Data = [byte]0 } -ErrorAction SilentlyContinue | Out-Null
                }
                "#
            }
            FanMode::Gaming => {
                r#"
                $set = Get-CimInstance -Namespace root/wmi -ClassName GB_WMIACPI_Set -ErrorAction SilentlyContinue | Select-Object -First 1
                if ($set) {
                    Invoke-CimMethod -InputObject $set -MethodName SetFanModeNotify -Arguments @{ Data = [byte]2 } -ErrorAction SilentlyContinue | Out-Null
                    Invoke-CimMethod -InputObject $set -MethodName SetTurboMode -Arguments @{ Data = [byte]0 } -ErrorAction SilentlyContinue | Out-Null
                    Invoke-CimMethod -InputObject $set -MethodName SetQuietMode -Arguments @{ Data = [byte]0 } -ErrorAction SilentlyContinue | Out-Null
                    Invoke-CimMethod -InputObject $set -MethodName SetFixedFanStatus -Arguments @{ Data = [byte]0 } -ErrorAction SilentlyContinue | Out-Null
                    Invoke-CimMethod -InputObject $set -MethodName SetStepFanStatus -Arguments @{ Data = [byte]0 } -ErrorAction SilentlyContinue | Out-Null
                    Invoke-CimMethod -InputObject $set -MethodName SetAutoFanStatus -Arguments @{ Data = [byte]1 } -ErrorAction SilentlyContinue | Out-Null
                    Invoke-CimMethod -InputObject $set -MethodName SetThermalTppStatus -Arguments @{ Data = [byte]1 } -ErrorAction SilentlyContinue | Out-Null
                }
                "#
            }
            FanMode::Turbo => {
                r#"
                $set = Get-CimInstance -Namespace root/wmi -ClassName GB_WMIACPI_Set -ErrorAction SilentlyContinue | Select-Object -First 1
                if ($set) {
                    Invoke-CimMethod -InputObject $set -MethodName SetFanModeNotify -Arguments @{ Data = [byte]3 } -ErrorAction SilentlyContinue | Out-Null
                    Invoke-CimMethod -InputObject $set -MethodName SetQuietMode -Arguments @{ Data = [byte]0 } -ErrorAction SilentlyContinue | Out-Null
                    Invoke-CimMethod -InputObject $set -MethodName SetTurboMode -Arguments @{ Data = [byte]1 } -ErrorAction SilentlyContinue | Out-Null
                    Invoke-CimMethod -InputObject $set -MethodName SetAutoFanStatus -Arguments @{ Data = [byte]0 } -ErrorAction SilentlyContinue | Out-Null
                    Invoke-CimMethod -InputObject $set -MethodName SetFixedFanStatus -Arguments @{ Data = [byte]1 } -ErrorAction SilentlyContinue | Out-Null
                    Invoke-CimMethod -InputObject $set -MethodName SetStepFanStatus -Arguments @{ Data = [byte]1 } -ErrorAction SilentlyContinue | Out-Null
                    Invoke-CimMethod -InputObject $set -MethodName SetFixedFanSpeed -Arguments @{ Data = [byte]229 } -ErrorAction SilentlyContinue | Out-Null
                    Invoke-CimMethod -InputObject $set -MethodName SetGpuFanDuty -Arguments @{ Data = [byte]229 } -ErrorAction SilentlyContinue | Out-Null
                    Invoke-CimMethod -InputObject $set -MethodName SetThermalTppStatus -Arguments @{ Data = [byte]1 } -ErrorAction SilentlyContinue | Out-Null
                }
                "#
            }
            FanMode::Custom => {
                return Ok(());
            }
        };

        run_powershell_silent(script);
        Ok(())
    }

    fn set_custom_fan_speed(&mut self, speed_percent: u8) -> Result<(), String> {
        let clamped = speed_percent.clamp(25, 100);
        let val = (57.0 + ((clamped - 25) as f64 / 75.0) * (229.0 - 57.0)) as u8;
        apply_pwm_speed(val);
        Ok(())
    }

    fn get_telemetry(&self) -> Result<TelemetryData, String> {
        fetch_telemetry()
    }

    fn set_battery_charge_limit(&mut self, enabled: bool) -> Result<(), String> {
        let charge_value = if enabled { 80 } else { 100 };
        let script = format!(
            r#"
            $set = Get-CimInstance -Namespace root/wmi -ClassName GB_WMIACPI_Set -ErrorAction SilentlyContinue | Select-Object -First 1
            if ($set) {{
                Invoke-CimMethod -InputObject $set -MethodName SetMaxCharge -Arguments @{{ Data = [byte]{} }} -ErrorAction SilentlyContinue | Out-Null
            }}
            "#,
            charge_value
        );

        run_powershell_silent(&script);
        Ok(())
    }

    fn get_battery_charge_limit(&self) -> Result<u8, String> {
        let script = r#"
            $get = Get-CimInstance -Namespace root/wmi -ClassName GB_WMIACPI_Get -ErrorAction SilentlyContinue | Select-Object -First 1
            if ($get) {
                $res = (Invoke-CimMethod -InputObject $get -MethodName GetMaxCharge -ErrorAction SilentlyContinue).Data
                if ($res -ne $null) {
                    Write-Output "$res"
                }
            }
        "#;

        let output = Command::new("powershell")
            .args(["-NoProfile", "-NonInteractive", "-Command", script])
            .creation_flags(CREATE_NO_WINDOW)
            .output()
            .map_err(|e| e.to_string())?;

        let text = String::from_utf8_lossy(&output.stdout).trim().to_string();
        if let Ok(val) = text.parse::<u8>() {
            Ok(val)
        } else {
            Err("Failed to read GetMaxCharge from WMI".to_string())
        }
    }

    fn get_gpu_mode(&self) -> Result<GpuMode, String> {
        let script = r#"
            $get = Get-CimInstance -Namespace root/wmi -ClassName GB_WMIACPI_Get -ErrorAction SilentlyContinue | Select-Object -First 1
            if ($get) {
                $res = (Invoke-CimMethod -InputObject $get -MethodName GetPEG2orSG2 -ErrorAction SilentlyContinue).Data
                if ($res -ne $null) {
                    Write-Output "$res"
                }
            }
        "#;

        let output = Command::new("powershell")
            .args(["-NoProfile", "-NonInteractive", "-Command", script])
            .creation_flags(CREATE_NO_WINDOW)
            .output()
            .map_err(|e| e.to_string())?;

        let text = String::from_utf8_lossy(&output.stdout).trim().to_string();
        if let Ok(val) = text.parse::<u8>() {
            Ok(GpuMode::from_u8(val))
        } else {
            Err("Failed to read MUX status from WMI".to_string())
        }
    }

    fn set_gpu_mode(&mut self, mode: GpuMode) -> Result<(), String> {
        let val = mode as u8;
        let script = format!(
            r#"
            $set = Get-CimInstance -Namespace root/wmi -ClassName GB_WMIACPI_Set -ErrorAction SilentlyContinue | Select-Object -First 1
            if ($set) {{
                Invoke-CimMethod -InputObject $set -MethodName SetPEG2orSG2 -Arguments @{{ Data = [byte]{} }} -ErrorAction SilentlyContinue | Out-Null
            }}
            "#,
            val
        );

        run_powershell_silent(&script);
        Ok(())
    }

    fn get_dgpu_status(&self) -> Result<bool, String> {
        let script = r#"
            $dev = Get-PnpDevice -Class Display | Where-Object { $_.Manufacturer -like '*NVIDIA*' -or $_.FriendlyName -like '*NVIDIA*' -or $_.InstanceId -like '*VEN_10DE*' } | Select-Object -First 1
            if ($dev) {
                if ($dev.Status -eq 'OK') {
                    Write-Output 'OK'
                } elseif ($dev.Status -eq 'Error' -or $dev.Status -eq 'Degraded' -or $dev.Problem -eq 22 -or $dev.ConfigManagerErrorCode -eq 22) {
                    Write-Output 'DISABLED'
                } else {
                    Write-Output 'OK'
                }
            }
        "#;

        let output = Command::new("powershell")
            .args(["-NoProfile", "-NonInteractive", "-Command", script])
            .creation_flags(CREATE_NO_WINDOW)
            .output()
            .map_err(|e| e.to_string())?;

        let text = String::from_utf8_lossy(&output.stdout).trim().to_string();
        if text == "OK" {
            Ok(true)
        } else if text == "DISABLED" {
            Ok(false)
        } else {
            Err("NVIDIA GPU device not found in Display class".to_string())
        }
    }

    fn set_dgpu_enabled(&mut self, enabled: bool) -> Result<(), String> {
        let script = if enabled {
            r#"
            $dev = Get-PnpDevice -Class Display | Where-Object { $_.Manufacturer -like '*NVIDIA*' -or $_.FriendlyName -like '*NVIDIA*' -or $_.InstanceId -like '*VEN_10DE*' } | Select-Object -First 1
            if ($dev) {
                Enable-PnpDevice -InstanceId $dev.InstanceId -Confirm:$false -ErrorAction SilentlyContinue | Out-Null
            }
            "#
        } else {
            r#"
            $dev = Get-PnpDevice -Class Display | Where-Object { $_.Manufacturer -like '*NVIDIA*' -or $_.FriendlyName -like '*NVIDIA*' -or $_.InstanceId -like '*VEN_10DE*' } | Select-Object -First 1
            if ($dev) {
                Disable-PnpDevice -InstanceId $dev.InstanceId -Confirm:$false -ErrorAction SilentlyContinue | Out-Null
            }
            "#
        };

        run_powershell_silent(script);
        Ok(())
    }

    fn get_display_hz(&self) -> u32 {
        unsafe {
            let mut dm: DEVMODEW = std::mem::zeroed();
            dm.dmSize = std::mem::size_of::<DEVMODEW>() as u16;

            if EnumDisplaySettingsW(std::ptr::null(), ENUM_CURRENT_SETTINGS, &mut dm) != 0 {
                dm.dmDisplayFrequency
            } else {
                165
            }
        }
    }

    fn set_display_hz(&mut self, hz: u32) -> Result<(), String> {
        unsafe {
            let mut dm: DEVMODEW = std::mem::zeroed();
            dm.dmSize = std::mem::size_of::<DEVMODEW>() as u16;

            if EnumDisplaySettingsW(std::ptr::null(), ENUM_CURRENT_SETTINGS, &mut dm) == 0 {
                return Err("Failed to query current display settings".to_string());
            }

            dm.dmFields = DM_DISPLAYFREQUENCY;
            dm.dmDisplayFrequency = hz;

            let res = ChangeDisplaySettingsExW(
                std::ptr::null(),
                &dm,
                std::ptr::null_mut(),
                CDS_UPDATEREGISTRY,
                std::ptr::null(),
            );

            if res == DISP_CHANGE_SUCCESSFUL {
                Ok(())
            } else {
                Err(format!("ChangeDisplaySettingsExW failed with code {}", res))
            }
        }
    }
}

// ==========================================
// 2. LIGHTING CONTROLLER (USB HID)
// ==========================================

pub struct WindowsLightingController {
    kb_paths: Vec<String>,
    bar_paths: Vec<String>,
    last_kb_color: Option<RgbColor>,
    last_kb_brightness: Option<u8>,
    last_bar_color: Option<RgbColor>,
    last_bar_brightness: Option<u8>,
}

impl WindowsLightingController {
    pub fn new() -> Self {
        let all_paths = unsafe { get_hid_device_paths() };
        let mut kb_paths = Vec::new();
        let mut bar_paths = Vec::new();

        for path in all_paths {
            let path_lower = path.to_lowercase();
            if path_lower.contains("vid_0414") && path_lower.contains("pid_8005") {
                kb_paths.push(path);
            } else if path_lower.contains("vid_0414") && path_lower.contains("pid_8012") {
                bar_paths.push(path);
            }
        }

        Self {
            kb_paths,
            bar_paths,
            last_kb_color: None,
            last_kb_brightness: None,
            last_bar_color: None,
            last_bar_brightness: None,
        }
    }
}

impl LightingController for WindowsLightingController {
    fn set_zone_lighting(&mut self, zone: RgbZone, color: RgbColor, brightness: u8) -> Result<(), String> {
        let color_code = color.to_u8();

        let hw_brightness = if brightness == 0 || color == RgbColor::Off {
            0
        } else {
            ((brightness as f32 / 100.0) * 50.0).round().clamp(1.0, 50.0) as u8
        };

        match zone {
            RgbZone::Keyboard => {
                if self.last_kb_color == Some(color) && self.last_kb_brightness == Some(hw_brightness) {
                    return Ok(());
                }
                self.last_kb_color = Some(color);
                self.last_kb_brightness = Some(hw_brightness);

                for path in &self.kb_paths {
                    if color == RgbColor::Off || hw_brightness == 0 {
                        let off_pkt = make_static_packet(0x08, 0, 0);
                        send_raw_packet(path, &off_pkt);
                    } else {
                        let pkt = make_static_packet(0x08, hw_brightness, color_code);
                        send_raw_packet(path, &pkt);
                    }
                }
            }
            RgbZone::RearBar => {
                if self.last_bar_color == Some(color) && self.last_bar_brightness == Some(hw_brightness) {
                    return Ok(());
                }
                self.last_bar_color = Some(color);
                self.last_bar_brightness = Some(hw_brightness);

                for path in &self.bar_paths {
                    if color == RgbColor::Off || hw_brightness == 0 {
                        let off_pkt1 = make_static_packet(0x08, 0, 0);
                        let off_pkt2 = make_static_packet(0x0B, 0, 0);
                        send_raw_packet(path, &off_pkt1);
                        send_raw_packet(path, &off_pkt2);
                    } else {
                        let pkt1 = make_static_packet(0x08, hw_brightness, color_code);
                        let pkt2 = make_static_packet(0x0B, hw_brightness, color_code);
                        send_raw_packet(path, &pkt1);
                        send_raw_packet(path, &pkt2);
                    }
                }
            }
        }

        Ok(())
    }
}

// ==========================================
// 3. LOW LEVEL HELPERS
// ==========================================

fn make_static_packet(cmd: u8, brightness: u8, color_idx: u8) -> [u8; 70] {
    let mut pkt = [0u8; 70];
    pkt[0] = 0x00;
    pkt[1] = cmd;
    pkt[2] = 0x00;
    pkt[3] = 0x01;
    pkt[4] = 0x01;
    pkt[5] = brightness;
    pkt[6] = color_idx;
    pkt[7] = 0x01;
    let sum: u32 = pkt[1..8].iter().map(|&x| x as u32).sum();
    pkt[8] = (0xFF - (sum & 0xFF)) as u8;
    pkt
}

unsafe fn get_hid_device_paths() -> Vec<String> {
    let mut guid = GUID { data1: 0, data2: 0, data3: 0, data4: [0; 8] };
    HidD_GetHidGuid(&mut guid);

    let h_dev_info = SetupDiGetClassDevsW(
        &guid,
        std::ptr::null(),
        std::ptr::null_mut(),
        DIGCF_PRESENT | DIGCF_DEVICEINTERFACE,
    );
    if h_dev_info == 0 || h_dev_info == INVALID_HANDLE_VALUE as isize {
        return Vec::new();
    }

    let mut paths = Vec::new();
    let mut dev_interface_data = SP_DEVICE_INTERFACE_DATA {
        cbSize: std::mem::size_of::<SP_DEVICE_INTERFACE_DATA>() as u32,
        InterfaceClassGuid: GUID { data1: 0, data2: 0, data3: 0, data4: [0; 8] },
        Flags: 0,
        Reserved: 0,
    };

    let mut index = 0;
    while SetupDiEnumDeviceInterfaces(
        h_dev_info,
        std::ptr::null_mut(),
        &guid,
        index,
        &mut dev_interface_data,
    ) != 0
    {
        let mut required_size = 0;
        SetupDiGetDeviceInterfaceDetailW(
            h_dev_info,
            &mut dev_interface_data,
            std::ptr::null_mut(),
            0,
            &mut required_size,
            std::ptr::null_mut(),
        );

        if required_size > 0 {
            let mut buffer: Vec<u8> = vec![0; required_size as usize];
            let detail = buffer.as_mut_ptr() as *mut SP_DEVICE_INTERFACE_DETAIL_DATA_W;
            (*detail).cbSize = 8;

            if SetupDiGetDeviceInterfaceDetailW(
                h_dev_info,
                &mut dev_interface_data,
                detail,
                required_size,
                std::ptr::null_mut(),
                std::ptr::null_mut(),
            ) != 0
            {
                let path_slice = std::slice::from_raw_parts(
                    (*detail).DevicePath.as_ptr(),
                    (required_size as usize - 4) / 2,
                );
                let path_len = path_slice.iter().position(|&c| c == 0).unwrap_or(path_slice.len());
                let path = String::from_utf16_lossy(&path_slice[..path_len]);
                paths.push(path);
            }
        }
        index += 1;
    }

    SetupDiDestroyDeviceInfoList(h_dev_info);
    paths
}

fn send_raw_packet(device_path: &str, packet: &[u8; 70]) {
    let wide: Vec<u16> = device_path.encode_utf16().chain(std::iter::once(0)).collect();
    unsafe {
        let handle = CreateFileW(
            wide.as_ptr(),
            GENERIC_READ | GENERIC_WRITE,
            FILE_SHARE_READ | FILE_SHARE_WRITE,
            std::ptr::null(),
            OPEN_EXISTING,
            0,
            std::ptr::null_mut(),
        );

        if handle != INVALID_HANDLE_VALUE as _ {
            HidD_SetFeature(handle as _, packet.as_ptr() as *const c_void, 70);
            CloseHandle(handle as _);
        }
    }
}

fn run_powershell_silent(script: &str) {
    let _ = Command::new("powershell")
        .args(["-NoProfile", "-NonInteractive", "-Command", script])
        .creation_flags(CREATE_NO_WINDOW)
        .spawn();
}
