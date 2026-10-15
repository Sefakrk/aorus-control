use std::ffi::c_void;
use std::os::windows::process::CommandExt;
use std::process::Command;
use windows_sys::core::GUID;
use windows_sys::Win32::Devices::DeviceAndDriverInstallation::*;
use windows_sys::Win32::Devices::HumanInterfaceDevice::*;
use windows_sys::Win32::Foundation::*;

use crate::hal::models::*;
use crate::hal::{LightingController, ThermalController};

const CREATE_NO_WINDOW: u32 = 0x08000000;

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
}

const GENERIC_READ: u32 = 0x80000000;
const GENERIC_WRITE: u32 = 0x40000000;
const FILE_SHARE_READ: u32 = 1;
const FILE_SHARE_WRITE: u32 = 2;
const OPEN_EXISTING: u32 = 3;

// ==========================================
// 1. THERMAL CONTROLLER (WMI ACPI)
// ==========================================

pub struct WindowsThermalController;

impl WindowsThermalController {
    pub fn new() -> Self {
        Self
    }
}

impl ThermalController for WindowsThermalController {
    fn set_fan_mode(&mut self, mode: FanMode) -> Result<(), String> {
        let script = match mode {
            FanMode::Quiet => {
                r#"
                $set = Get-CimInstance -Namespace root/wmi -ClassName GB_WMIACPI_Set -ErrorAction SilentlyContinue
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
                $set = Get-CimInstance -Namespace root/wmi -ClassName GB_WMIACPI_Set -ErrorAction SilentlyContinue
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
                $set = Get-CimInstance -Namespace root/wmi -ClassName GB_WMIACPI_Set -ErrorAction SilentlyContinue
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
                $set = Get-CimInstance -Namespace root/wmi -ClassName GB_WMIACPI_Set -ErrorAction SilentlyContinue
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
            FanMode::Manual(percent) => {
                return self.set_custom_fan_speed(percent);
            }
        };

        run_powershell_silent(script);
        Ok(())
    }

    fn set_custom_fan_speed(&mut self, speed_percent: u8) -> Result<(), String> {
        let clamped = speed_percent.clamp(25, 100);
        let val = (57.0 + ((clamped - 25) as f64 / 75.0) * (229.0 - 57.0)) as u8;

        let script = format!(
            r#"
            $set = Get-CimInstance -Namespace root/wmi -ClassName GB_WMIACPI_Set -ErrorAction SilentlyContinue
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
            val, val
        );

        run_powershell_silent(&script);
        Ok(())
    }

    fn get_telemetry(&self) -> Result<TelemetryData, String> {
        let script = r#"
            $get = Get-CimInstance -Namespace root/wmi -ClassName GB_WMIACPI_Get -ErrorAction SilentlyContinue
            if ($get) {
                $cpu = (Invoke-CimMethod -InputObject $get -MethodName getCpuTemp -ErrorAction SilentlyContinue).Data
                $gpu = (Invoke-CimMethod -InputObject $get -MethodName getGpuTemp1 -ErrorAction SilentlyContinue).Data
                $rpm1 = (Invoke-CimMethod -InputObject $get -MethodName getRpm1 -ErrorAction SilentlyContinue).Data
                $rpm2 = (Invoke-CimMethod -InputObject $get -MethodName getRpm2 -ErrorAction SilentlyContinue).Data
                Write-Output "$cpu,$gpu,$rpm1,$rpm2"
            }
        "#;

        let output = Command::new("powershell")
            .args(["-NoProfile", "-NonInteractive", "-Command", script])
            .creation_flags(CREATE_NO_WINDOW)
            .output()
            .map_err(|e| e.to_string())?;

        let text = String::from_utf8_lossy(&output.stdout).trim().to_string();
        let parts: Vec<&str> = text.split(',').collect();

        if parts.len() >= 4 {
            let cpu = parts[0].trim().parse::<u8>().unwrap_or(50);
            let gpu = parts[1].trim().parse::<u8>().unwrap_or(45);
            let raw1 = parts[2].trim().parse::<u32>().unwrap_or(0);
            let raw2 = parts[3].trim().parse::<u32>().unwrap_or(0);

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

            Ok(TelemetryData {
                cpu_temp: cpu,
                gpu_temp: gpu,
                fan1_rpm: swap_rpm(raw1),
                fan2_rpm: swap_rpm(raw2),
            })
        } else {
            Ok(TelemetryData {
                cpu_temp: 50,
                gpu_temp: 45,
                fan1_rpm: 2200,
                fan2_rpm: 2200,
            })
        }
    }

    fn set_battery_charge_limit(&mut self, enabled: bool) -> Result<(), String> {
        let charge_value = if enabled { 80 } else { 100 };
        let script = format!(
            r#"
            $set = Get-CimInstance -Namespace root/wmi -ClassName GB_WMIACPI_Set -ErrorAction SilentlyContinue
            if ($set) {{
                Invoke-CimMethod -InputObject $set -MethodName SetMaxCharge -Arguments @{{ Data = [byte]{} }} -ErrorAction SilentlyContinue | Out-Null
            }}
            "#,
            charge_value
        );

        run_powershell_silent(&script);
        Ok(())
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
        // Enumerate HID devices ONCE on startup and cache paths
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

        match zone {
            RgbZone::Keyboard => {
                // Deduplicate redundant packet writes
                if self.last_kb_color == Some(color) && self.last_kb_brightness == Some(brightness) {
                    return Ok(());
                }
                self.last_kb_color = Some(color);
                self.last_kb_brightness = Some(brightness);

                for path in &self.kb_paths {
                    if color == RgbColor::Off || brightness == 0 {
                        let off_pkt = make_static_packet(0x08, 0, 0);
                        send_raw_packet(path, &off_pkt);
                    } else {
                        let pkt = make_static_packet(0x08, brightness, color_code);
                        send_raw_packet(path, &pkt);
                    }
                }
            }
            RgbZone::RearBar => {
                // Deduplicate redundant packet writes
                if self.last_bar_color == Some(color) && self.last_bar_brightness == Some(brightness) {
                    return Ok(());
                }
                self.last_bar_color = Some(color);
                self.last_bar_brightness = Some(brightness);

                for path in &self.bar_paths {
                    if color == RgbColor::Off || brightness == 0 {
                        let off_pkt1 = make_static_packet(0x08, 0, 0);
                        let off_pkt2 = make_static_packet(0x0B, 0, 0);
                        send_raw_packet(path, &off_pkt1);
                        send_raw_packet(path, &off_pkt2);
                    } else {
                        let pkt1 = make_static_packet(0x08, brightness, color_code);
                        let pkt2 = make_static_packet(0x0B, 0x64, color_code);
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
    pkt[3] = 0x01; // Pure Solid Static
    pkt[4] = 0x01; // Speed 1
    pkt[5] = brightness;
    pkt[6] = color_idx;
    pkt[7] = 0x01; // Direction 1
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
