use std::ffi::c_void;
use windows_sys::core::GUID;
use windows_sys::Win32::Devices::DeviceAndDriverInstallation::*;
use windows_sys::Win32::Devices::HumanInterfaceDevice::*;
use windows_sys::Win32::Foundation::*;

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

unsafe fn get_hid_device_paths() -> Vec<String> {
    let mut guid = GUID { data1: 0, data2: 0, data3: 0, data4: [0; 8] };
    HidD_GetHidGuid(&mut guid);

    let h_dev_info = SetupDiGetClassDevsW(
        &guid,
        std::ptr::null(),
        std::ptr::null_mut(),
        DIGCF_PRESENT | DIGCF_DEVICEINTERFACE,
    );
    if h_dev_info == INVALID_HANDLE_VALUE as _ || h_dev_info == 0 as _ {
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

// Pure static effect packet: type=1 (Static), speed=1, brightness=0..100, color=0..7, dir=1
fn make_static_packet(cmd: u8, brightness: u8, color_idx: u8) -> [u8; 70] {
    let mut pkt = [0u8; 70];
    pkt[0] = 0x00; // Report ID
    pkt[1] = cmd;  // 0x08 for keyboard/lightbar, 0x0B for strip/logo
    pkt[2] = 0x00;
    pkt[3] = 0x01; // Type 1 = PURE SOLID STATIC!
    pkt[4] = 0x01; // Speed 1
    pkt[5] = brightness;
    pkt[6] = color_idx;
    pkt[7] = 0x01; // Direction 1
    let sum: u32 = pkt[1..8].iter().map(|&x| x as u32).sum();
    pkt[8] = (0xFF - (sum & 0xFF)) as u8;
    pkt
}

// KEYBOARD: PID 8005
pub fn set_keyboard_color(brightness: u8, color_idx: u8) {
    let all_paths = unsafe { get_hid_device_paths() };

    for path in all_paths {
        let path_lower = path.to_lowercase();
        if path_lower.contains("vid_0414") && path_lower.contains("pid_8005") {
            let pkt = make_static_packet(0x08, brightness, color_idx);
            send_raw_packet(&path, &pkt);
        }
    }
}

// REAR LIGHTBAR: PID 8012 (controls all 3 parts: Bar, Strip, Logo)
pub fn set_lightbar_color(enabled: bool, color_idx: u8) {
    let all_paths = unsafe { get_hid_device_paths() };

    for path in all_paths {
        let path_lower = path.to_lowercase();
        if path_lower.contains("vid_0414") && path_lower.contains("pid_8012") {
            if enabled && color_idx > 0 {
                // Send both 0x08 (bar) and 0x0B (strip and logo)
                let pkt1 = make_static_packet(0x08, 100, color_idx);
                let pkt2 = make_static_packet(0x0B, 100, color_idx);
                send_raw_packet(&path, &pkt1);
                send_raw_packet(&path, &pkt2);
            } else {
                let off_pkt1 = make_static_packet(0x08, 0, 0);
                let off_pkt2 = make_static_packet(0x0B, 0, 0);
                send_raw_packet(&path, &off_pkt1);
                send_raw_packet(&path, &off_pkt2);
            }
        }
    }
}

pub fn turn_keyboard_off() {
    set_keyboard_color(0, 0);
}

pub fn turn_lightbar_off() {
    set_lightbar_color(false, 0);
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
