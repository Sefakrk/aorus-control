#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod config;
mod hal;

use std::sync::Arc;
use std::time::Duration;
use parking_lot::Mutex;
use config::AppConfig;
use hal::models::*;
use hal::create_controllers;

slint::include_modules!();

fn main() -> Result<(), slint::PlatformError> {
    let main_window = AppWindow::new()?;

    // 1. Load persisted configuration
    let config = Arc::new(Mutex::new(AppConfig::load()));

    // 2. Initialize HAL controllers
    let (thermal_ctrl, lighting_ctrl) = create_controllers();
    let thermal_controller = Arc::new(Mutex::new(thermal_ctrl));
    let lighting_controller = Arc::new(Mutex::new(lighting_ctrl));

    // 3. Apply persisted configuration to hardware and UI immediately on startup
    {
        let cfg = config.lock();

        let mode_str = match cfg.fan_mode {
            FanMode::Quiet => "Quiet",
            FanMode::Gaming => "Gaming",
            FanMode::Turbo => "Turbo",
            FanMode::Manual(_) => "Manual",
            _ => "Normal",
        };
        main_window.set_active_mode(mode_str.into());
        main_window.set_fan_speed(cfg.manual_fan_speed as f32);
        main_window.set_kb_brightness(cfg.kb_brightness as f32);
        main_window.set_active_kb_color(cfg.kb_color.to_name().into());
        main_window.set_active_bar_color(cfg.bar_color.to_name().into());
        main_window.set_lightbar_enabled(cfg.bar_enabled);
        main_window.set_battery_limit(cfg.battery_limit);

        // Apply to hardware via HAL
        let mut tc = thermal_controller.lock();
        let _ = tc.set_fan_mode(cfg.fan_mode);
        let _ = tc.set_battery_charge_limit(cfg.battery_limit);

        let mut lc = lighting_controller.lock();
        let _ = lc.set_zone_lighting(RgbZone::Keyboard, cfg.kb_color, cfg.kb_brightness);
        let bar_color = if cfg.bar_enabled { cfg.bar_color } else { RgbColor::Off };
        let _ = lc.set_zone_lighting(RgbZone::RearBar, bar_color, 100);
    }

    // 4. Keyboard Color Callback
    {
        let lc_clone = lighting_controller.clone();
        let cfg_clone = config.clone();

        main_window.on_set_keyboard_color(move |preset| {
            let color = RgbColor::from_name(preset.as_str());
            let brightness = {
                let mut cfg = cfg_clone.lock();
                cfg.kb_color = color;
                let _ = cfg.save();
                cfg.kb_brightness
            };

            let mut lc = lc_clone.lock();
            let _ = lc.set_zone_lighting(RgbZone::Keyboard, color, brightness);
        });
    }

    // 5. Keyboard Brightness Live Drag Callback (Fast lighting update, zero disk I/O)
    {
        let lc_clone = lighting_controller.clone();
        let cfg_clone = config.clone();

        main_window.on_set_keyboard_brightness(move |val| {
            let br = val.clamp(0.0, 100.0) as u8;
            let color = cfg_clone.lock().kb_color;

            let mut lc = lc_clone.lock();
            let _ = lc.set_zone_lighting(RgbZone::Keyboard, color, br);
        });
    }

    // 6. Keyboard Brightness Save Callback (Saves to disk only on slider release)
    {
        let cfg_clone = config.clone();

        main_window.on_save_keyboard_brightness(move |val| {
            let br = val.clamp(0.0, 100.0) as u8;
            let mut cfg = cfg_clone.lock();
            cfg.kb_brightness = br;
            let _ = cfg.save();
        });
    }

    // 7. Lightbar Color Callback
    {
        let lc_clone = lighting_controller.clone();
        let cfg_clone = config.clone();

        main_window.on_set_lightbar_color(move |preset| {
            let color = RgbColor::from_name(preset.as_str());
            let enabled = {
                let mut cfg = cfg_clone.lock();
                cfg.bar_color = color;
                let _ = cfg.save();
                cfg.bar_enabled
            };

            let mut lc = lc_clone.lock();
            let apply_color = if enabled { color } else { RgbColor::Off };
            let _ = lc.set_zone_lighting(RgbZone::RearBar, apply_color, 100);
        });
    }

    // 8. Lightbar Toggle Callback
    {
        let lc_clone = lighting_controller.clone();
        let cfg_clone = config.clone();

        main_window.on_toggle_lightbar(move |enabled| {
            let color = {
                let mut cfg = cfg_clone.lock();
                cfg.bar_enabled = enabled;
                let _ = cfg.save();
                cfg.bar_color
            };

            let mut lc = lc_clone.lock();
            let apply_color = if enabled { color } else { RgbColor::Off };
            let _ = lc.set_zone_lighting(RgbZone::RearBar, apply_color, 100);
        });
    }

    // 9. Fan Mode Callback
    {
        let tc_clone = thermal_controller.clone();
        let cfg_clone = config.clone();

        main_window.on_set_fan_mode(move |mode_str| {
            let mode = match mode_str.as_str() {
                "Quiet" => FanMode::Quiet,
                "Gaming" => FanMode::Gaming,
                "Turbo" => FanMode::Turbo,
                _ => FanMode::Normal,
            };

            {
                let mut cfg = cfg_clone.lock();
                cfg.fan_mode = mode;
                let _ = cfg.save();
            }

            let mut tc = tc_clone.lock();
            let _ = tc.set_fan_mode(mode);
        });
    }

    // 10. Custom Fan Speed Callback
    {
        let tc_clone = thermal_controller.clone();
        let cfg_clone = config.clone();

        main_window.on_set_custom_fan(move |speed| {
            let clamped = speed.clamp(25, 100) as u8;

            {
                let mut cfg = cfg_clone.lock();
                cfg.fan_mode = FanMode::Manual(clamped);
                cfg.manual_fan_speed = clamped;
                let _ = cfg.save();
            }

            let mut tc = tc_clone.lock();
            let _ = tc.set_custom_fan_speed(clamped);
        });
    }

    // 11. Battery Limit Callback
    {
        let tc_clone = thermal_controller.clone();
        let cfg_clone = config.clone();

        main_window.on_toggle_battery_limit(move |enabled| {
            {
                let mut cfg = cfg_clone.lock();
                cfg.battery_limit = enabled;
                let _ = cfg.save();
            }

            let mut tc = tc_clone.lock();
            let _ = tc.set_battery_charge_limit(enabled);
        });
    }

    // 12. Non-blocking Background Telemetry Thread
    // Does NOT hold the thermal_controller Mutex lock during WMI execution
    // Sleep is at the END of the loop so initial telemetry appears instantly
    {
        let window_weak = main_window.as_weak();

        std::thread::spawn(move || {
            loop {
                // Fetch directly without locking the ThermalController mutex
                if let Ok(telemetry) = hal::windows::fetch_telemetry() {
                    let weak = window_weak.clone();
                    let _ = slint::invoke_from_event_loop(move || {
                        if let Some(window) = weak.upgrade() {
                            window.set_cpu_temp(format!("{} \u{00B0}C", telemetry.cpu_temp).into());
                            window.set_gpu_temp(format!("{} \u{00B0}C", telemetry.gpu_temp).into());
                            let rpm = if telemetry.fan1_rpm > 0 { telemetry.fan1_rpm } else { telemetry.fan2_rpm };
                            window.set_fan_rpm(format!("{} RPM", rpm).into());
                        }
                    });
                }

                // Sleep at the end of the loop
                std::thread::sleep(Duration::from_millis(2000));
            }
        });
    }

    main_window.run()
}
