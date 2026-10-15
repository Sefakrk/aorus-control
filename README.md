# AORUS Control (for AORUS 16X 2024)

Lightweight, native, open-source replacement for **Gigabyte Control Center (GCC)** and **RGB Fusion** on Gigabyte AORUS 16X (2024, 9KG/9SG/AKG/ASG) laptops.

Built with **Rust** and **Slint UI** (GPU-accelerated, native Windows/Linux compatible).

---

## 🚀 Features

- **⚡ Zero Bloat:** ~15 MB RAM usage (vs GCC's 500+ MB), 0.0% CPU overhead, no background services or telemetry.
- **🎮 No Game Micro-Stutter:** No runtime garbage collector (GC); won't cause 1% low FPS drops in games.
- **⌨️ 100% Solid Static Keyboard RGB:** Hardware-level static lock, no flicker, no breathing, no unexpected effect switches.
- **✨ 3-Zone Rear LightBar (Aorus Beacon):** Controls all 3 rear lighting components (Main Bar, Exhaust Strip, Projection Logo) simultaneously.
- **🌪️ Fan & Power Modes:** Quiet, Normal, Gaming, Turbo + Manual Fan Duty Slider (30% - 100%).
- **📊 Real-Time Hardware Sensors:** CPU temperature, GPU (RTX 4060) temperature, and fan RPM (with big-endian byte-swap decoding).
- **🔋 Battery Health Care:** 80% maximum charge conservation limit.

---

## 🛠️ Hardware Reverse Engineering & Protocol Specs

### 1. Keyboard RGB
- **Device:** `HID\VID_0414&PID_8005`
- **Interface:** Windows Standard User-Space HID API (`HidD_SetFeature`)
- **Packet Structure (70 Bytes):**
  - Byte 0: `0x00` (Report ID)
  - Byte 1: `0x08` (Command: Set Light Effect / Color)
  - Byte 2: `0x00`
  - Byte 3: `0x01` (Type: 1 = Pure Solid Static)
  - Byte 4: `0x01` (Speed)
  - Byte 5: `Brightness` (0 - 100)
  - Byte 6: `Color Index` (1=Red, 2=Green, 3=Yellow, 4=Blue, 5=Orange, 6=Purple, 7=White, 0=Off)
  - Byte 7: `0x01` (Direction)
  - Byte 8: `Checksum` = `0xFF - (Sum(Bytes 1..7) & 0xFF)`
  - Bytes 9..69: `0x00`

### 2. Rear Lightbar (3 Zones: Main Bar, Strip, Aorus Beacon Logo)
- **Device:** `HID\VID_0414&PID_8012`
- **Commands:**
  - `0x08`: Main Lightbar
  - `0x0B`: LED Strip and Projection Logo
- **Static Lock:** Both `0x08` and `0x0B` packets are sent with `Type = 1` to prevent breathing pulsation across all 3 zones.

### 3. Fan & Power Profiles
- **Namespace:** `root\WMI`
- **WMI Classes:**
  - `GB_WMIACPI_Set`: All hardware modification methods (requires `Data = [byte]...` input argument)
  - `GB_WMIACPI_Get`: Real-time sensor readings (`getCpuTemp`, `getGpuTemp1`, `getRpm1`)
- **RPM Decoding:** Raw RPM value is transmitted as big-endian 16-bit integer; formula: `((raw & 0xFF) << 8) | ((raw >> 8) & 0xFF)`.

---

## 📦 Project Structure

```
aorus-control/
├── Cargo.toml          # Rust package config (Slint + Windows-sys)
├── build.rs            # Slint UI build script
├── ui/
│   └── appwindow.slint # Declarative modern dark UI layout
├── src/
│   ├── main.rs         # Application entry point & state management
│   ├── rgb.rs          # Low-level HID USB packet sender
│   └── hardware.rs     # WMI ACPI fan & battery hardware controller
└── README.md
```

---

## 🔨 How to Build

Requirements:
- Rust 1.80+ (with GNU or MSVC toolchain)
- MinGW-w64 or C++ build tools

```bash
cargo build --release
```

The compiled binary will be located at `target/release/aorus-control.exe`.
Run as Administrator to allow WMI hardware access.
