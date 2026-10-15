# AORUS 16X Control - Linux Porting Guide & Architecture Handover

> **Target Device:** Gigabyte AORUS 16X (2024 - 9KG / 9SG / 9XG Series)  
> **Repository:** [https://github.com/Sefakrk/aorus-control](https://github.com/Sefakrk/aorus-control)  
> **Tech Stack:** Rust (2021 Edition), Slint UI (v1.18, Zinc Minimal shadcn theme), Hardware Abstraction Layer (HAL).  
> **Author:** Antigravity / Sefakrk  

---

## 1. Executive Summary & Design Philosophy

`aorus-control` is a lightweight, zero-bloat, open-source native Rust replacement for **Gigabyte Control Center (GCC)** and **RGB Fusion**.
* **Zero Background Bloat:** No telemetry daemons, no memory leaks, 15 MB RAM footprint.
* **Pure Hardware Abstraction Layer (HAL):** All business logic, UI events, persistence (`aorus_config.json`), and custom curve hysteresis algorithms live in cross-platform code (`src/main.rs`, `src/config.rs`, `src/hal/models.rs`).
* **Cross-Platform UI:** The frontend is written in **Slint** (`ui/appwindow.slint`), which compiles natively to Wayland and X11 without webviews, Electron, or CSS engines.
* **Goal of the Linux Port:** Implement `src/hal/linux/mod.rs` to satisfy the `ThermalController` and `LightingController` traits defined in `src/hal/mod.rs`.

---

## 2. Project Architecture & File Tree

```text
aorus-control/
├── Cargo.toml                  # Dependencies (Needs target cfg separation)
├── build.rs                    # Slint compiler & Windows winres (needs linux cfg)
├── ARCHITECTURE.md             # Reverse-engineering notes & EC register map
├── LINUX_PORTING_GUIDE.md      # THIS DOCUMENT
├── aorus_config.json           # Local config persistence (JSON)
├── ui/
│   └── appwindow.slint         # Native UI (100% cross-platform, Wayland/X11 ready)
├── assets/
│   ├── icon.png & icon.ico     # App icons
│   ├── icon.rgba               # 256x256 raw RGBA for tray
│   └── acpi.bmof               # Windows WMI binary MOF (Windows only)
└── src/
    ├── main.rs                 # Event loop, Slint callbacks, custom curve engine
    ├── config.rs               # AppConfig (JSON load/save)
    └── hal/
        ├── mod.rs              # Hardware traits & factory (ThermalController, LightingController)
        ├── models.rs           # Pure data structs (FanMode, GpuMode, RgbColor, TelemetryData)
        ├── windows/            # Existing Windows implementation (Win32, HID, WMI)
        └── linux/              # <-- YOUR TARGET: Implement Linux HAL here!
            └── mod.rs
```

---

## 3. Reverse-Engineered Protocols (Ground Truth from Windows)

You do **NOT** need to reverse-engineer anything from scratch. Everything below has been dumped, decompiled from Gigabyte's `.dll` files (`ucNotebook.dll`, `KeyboardModel2023.dll`), and verified on real hardware.

### A. RGB Lighting (Keyboard & AORUS Beacon Lightbar)
* **Interface:** USB Human Interface Device (HID).
* **Target USB Identifiers:**
  * Keyboard (3-Zone RGB): `VID_0414 & PID_8005`
  * Rear Lightbar (AORUS Beacon & logo projector): `VID_0414 & PID_8012`
* **Transport:** 70-byte HID Feature Report (`Report ID: 0x00`).
* **Packet Size:** Exactly 70 bytes (`packet[0..70]`).
* **Packet Structure:**
  ```text
  packet[0] = 0x00 (Report ID)
  packet[1] = 0x08 (Static Color Effect Command)
  packet[2] = 0x00
  packet[3] = Zone Index (0x00 = Zone 0, 0x01 = Zone 1, 0x02 = Zone 2)
  packet[4] = Reserved (0x00)
  packet[5] = Reserved (0x00)
  packet[6] = Reserved (0x00)
  packet[7] = Color ID (1..7, or 0 = Off)
  packet[8] = Brightness (0..50)
  packet[9..68] = 0x00
  packet[69] = Checksum
  ```
* **CRITICAL FIRMWARE TRAPS (Pay Close Attention):**
  1. **Brightness Scaling is `0..50`, NOT `0..100`!**  
     Gigabyte firmware accepts brightness between `0x00` and `0x32` (`50`). If you send `51..100` (e.g. `100`), the keyboard firmware rejects the packet and **TURNS OFF THE LEDS!**  
     Formula: `hw_brightness = if ui_br == 0 { 0 } else { (ui_br as f32 / 100.0 * 50.0).round().clamp(1.0, 50.0) as u8 }`.
  2. **Checksum Calculation:**  
     `checksum = 0xFF - (sum(packet[1..=7]) & 0xFF)`.
  3. **Color Index Enum:**  
     `Off=0`, `Red=1`, `Green=2`, `Yellow=3`, `Blue=4`, `Orange=5`, `Purple=6`, `White=7`.
  4. **Rear Lightbar (PID_8012):**  
     The rear bar has 2 addressable sub-strips: `Zone 0` (Rear exhaust strip) and `Zone 1` (AORUS logo projector). To set color, send packets to both Zone 0 and Zone 1, followed by a wake/effect packet with `packet[1] = 0x0B`.

---

### B. Thermal, Fan Control & Telemetry

On Linux, the embedded controller (EC) registers of the AORUS 16X are mapped to the standard ITE IT5570 / IT8528 embedded controller.

#### 1. EC Direct Register Map (Discovered via `Vaider7/GigaCenter` & Gigabyte ACPI):
* **Fan Mode Control:** Register `0x08`
  * `0x00`: Normal Mode (Balanced)
  * `0x02`: Quiet Mode (Silent, fans off below 56°C)
  * `0x03`: Gaming Mode (Aggressive curve)
  * `0x04`: Turbo Mode (100% Fan PWM)
* **Manual / Fixed Fan Speed (PWM):**
  * CPU Fan: Register `0xFC`
  * GPU Fan: Register `0xFE`
  * Value: `0x00` (Fan Off) to `0xE5` (229 = 100% PWM).
* **Fan RPM Reading (Big-Endian 16-bit words):**
  * CPU Fan RPM: Registers `0x60` (High) & `0x61` (Low)
  * GPU Fan RPM: Registers `0x39` (High) & `0x3A` (Low)
  * *Note on Byte Swapping:* Firmware stores RPM in Big Endian. If raw value is `0x520A`, swapping bytes yields `0x0A52` = 2642 RPM.
* **Battery Charge Limit (%80 Threshold):**
  * Register `0xA9`: Write `0x50` (80 decimal) to lock charging at 80%. Write `0x64` (100 decimal) for standard charging.

#### 2. Factory Preset Fan Curves (PWM 25..100% vs Temp):
| Temperature | Quiet | Normal (Default) | Gaming | Turbo |
| :--- | :--- | :--- | :--- | :--- |
| **50 °C** | 0% (Fan Off) | 30% (~2200 RPM) | 35% (~2400 RPM) | 100% (5400 RPM) |
| **60 °C** | 22% (~1900 RPM) | 40% (~2700 RPM) | 45% (~3000 RPM) | 100% (5400 RPM) |
| **70 °C** | 28% (~2100 RPM) | 55% (~3300 RPM) | 70% (~4500 RPM) | 100% (5400 RPM) |
| **80 °C** | 35% (~2400 RPM) | 65% (~4200 RPM) | 85% (~4800 RPM) | 100% (5400 RPM) |
| **90 °C** | 45% (~3000 RPM) | 95% (~5100 RPM) | 100% (~5400 RPM) | 100% (5400 RPM) |

---

## 4. Suggested Implementation Paths on Linux

You have full creative freedom to choose the best and most idiomatic Linux implementation. Here are the recommended routes:

### Approach A: The `hidraw` + `hwmon` + `ec_sys` Route (Recommended & Most Reliable)

1. **RGB Control (`/dev/hidraw*`):**
   * Use the standard `hidapi` crate (`hidapi = "2.4"` in `Cargo.toml`).
   * Enumerate HID devices matching `vendor_id = 0x0414` and `product_id = 0x8005` or `0x8012`.
   * Call `device.send_feature_report(&packet)`.
   * *Permissions:* Provide a udev rule `/etc/udev/rules.d/99-aorus-control.rules`:
     ```udev
     KERNEL=="hidraw*", ATTRS{idVendor}=="0414", ATTRS{idProduct}=="8005", MODE="0666"
     KERNEL=="hidraw*", ATTRS{idVendor}=="0414", ATTRS{idProduct}=="8012", MODE="0666"
     ```

2. **Telemetry (CPU / GPU Temperature):**
   * Read from the Linux kernel's standard `hwmon` subsystem:
     * CPU: `/sys/class/hwmon/hwmon*/temp1_input` (divide by 1000 for °C).
     * GPU: Query via standard `nvidia-smi --query-gpu=temperature.gpu,utilization.gpu --format=csv,noheader,nounits` or read `/sys/class/drm/card*/device/hwmon/hwmon*/temp1_input`.
     * CPU Usage: Read `/proc/stat`.

3. **Fan Speed & Modes (`ec_sys` module):**
   * The kernel provides `ec_sys` (`modprobe ec_sys write_support=1`).
   * When loaded, `/sys/kernel/debug/ec/ec0/io` allows reading and writing raw EC registers directly from user-space!
   * Seek to `0x08` and write mode byte (`0` for Normal, `2` for Quiet, `3` for Gaming, `4` for Turbo).
   * For Custom PWM speed, seek to `0xFC` (Fan 1) and `0xFE` (Fan 2) and write the target PWM byte.

---

### Approach B: Standard Linux Subsystems (No `ec_sys` if kernel exposes WMI)
* Check if the Linux kernel module `gigabyte-wmi` is loaded.
* If present, inspect `/sys/devices/platform/gigabyte-wmi/` or `/sys/class/thermal/` for fan policies.
* For battery limit: Check `/sys/class/power_supply/BAT0/charge_control_limit_max`.

---

### Approach C: Display Refresh Rate (Hz) & GPU MUX on Linux
* **Refresh Rate (60Hz / 165Hz):**
  * Under Wayland: Call `wlr-randr` or Gnome/KDE D-Bus display interfaces.
  * Under X11: Call `xrandr --output <eDP-1> --mode 2560x1600 --rate 60` / `--rate 165`.
* **GPU Switching (MUX):**
  * Integrate with `supergfxctl` (D-Bus) or `envycontrol` CLI, or toggle `/sys/bus/pci/devices/0000:01:00.0/power/control`.

---

## 5. Contract: The Two Traits You Must Implement

In `src/hal/mod.rs`, implement these two traits inside `src/hal/linux/mod.rs`:

```rust
pub trait ThermalController: Send + Sync {
    fn set_fan_mode(&mut self, mode: FanMode) -> Result<(), String>;
    fn set_custom_fan_speed(&mut self, speed_percent: u8) -> Result<(), String>;
    fn get_telemetry(&self) -> Result<TelemetryData, String>;
    fn set_battery_charge_limit(&mut self, enabled: bool) -> Result<(), String>;
    fn get_battery_charge_limit(&self) -> Result<u8, String>;
    fn get_gpu_mode(&self) -> Result<GpuMode, String>;
    fn set_gpu_mode(&mut self, mode: GpuMode) -> Result<(), String>;
    fn get_dgpu_status(&self) -> Result<bool, String>;
    fn set_dgpu_enabled(&mut self, enabled: bool) -> Result<(), String>;
    fn get_display_hz(&self) -> u32;
    fn set_display_hz(&mut self, hz: u32) -> Result<(), String>;
}

pub trait LightingController: Send + Sync {
    fn set_zone_lighting(&mut self, zone: RgbZone, color: RgbColor, brightness: u8) -> Result<(), String>;
}
```

And in `src/hal/mod.rs`:
```rust
#[cfg(target_os = "windows")]
pub mod windows;
#[cfg(target_os = "linux")]
pub mod linux;

pub fn create_controllers() -> (Box<dyn ThermalController>, Box<dyn LightingController>) {
    #[cfg(target_os = "windows")]
    {
        (
            Box::new(windows::WindowsThermalController::new()),
            Box::new(windows::WindowsLightingController::new()),
        )
    }
    #[cfg(target_os = "linux")]
    {
        (
            Box::new(linux::LinuxThermalController::new()),
            Box::new(linux::LinuxLightingController::new()),
        )
    }
}
```

---

## 6. Cargo.toml Configuration (Platform Separation)

In `Cargo.toml`, isolate Windows-specific crates:

```toml
[dependencies]
slint = "1.18"
serde = { version = "1.0", features = ["derive"] }
serde_json = "1.0"
parking_lot = "0.12"
tray-icon = "0.19"

# Windows-only dependencies
[target.'cfg(target_os = "windows")'.dependencies]
windows-sys = { version = "0.59", features = [
    "Win32_Devices_HumanInterfaceDevice",
    "Win32_Devices_DeviceAndDriverInstallation",
    "Win32_Foundation",
    "Win32_Storage_FileSystem",
    "Win32_System_IO",
    "Win32_Graphics_Gdi",
] }

# Linux-only dependencies
[target.'cfg(target_os = "linux")'.dependencies]
hidapi = "2.4"
# Add any Linux-specific crates you prefer (e.g. zbus for D-Bus)

[build-dependencies]
slint-build = "1.18"

[target.'cfg(target_os = "windows")'.build-dependencies]
winres = "0.1"
```

In `build.rs`:
```rust
fn main() {
    slint_build::compile("ui/appwindow.slint").unwrap();

    #[cfg(target_os = "windows")]
    {
        let mut res = winres::WindowsResource::new();
        res.set_icon("assets/icon.ico");
        res.compile().unwrap();
    }
}
```

---

## 7. Golden Rules & Non-Goals

1. **DO NOT modify `ui/appwindow.slint`:** The UI is complete, styled with Zinc Minimal shadcn theme, and runs identically on Linux via Slint's winit backend.
2. **DO NOT change `src/main.rs` business logic:** The Slint callback bindings, hysteresis fan loop, and config serialization must remain intact.
3. **Graceful Fallbacks:** If a feature cannot be executed (e.g., dGPU kill switch on a system without `supergfxctl`, or `ec_sys` not loaded), return `Err("Descriptive message")` or a safe fallback so the UI stays responsive without crashing.
4. **Permissions:** Document the required udev rules and kernel modules (`ec_sys`) so the end-user can run `aorus-control` without running the entire GUI as `root`.
