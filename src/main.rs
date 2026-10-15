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

const PWM_TABLE: [(u8, u8); 16] = [
    (25, 57),  (30, 68),  (35, 80),  (40, 91),
    (45, 103), (50, 114), (55, 125), (60, 137),
    (65, 148), (70, 160), (75, 171), (80, 183),
    (85, 194), (90, 206), (95, 217), (100, 229),
];

fn interpolate_curve(temp: u8, curve: &[u8; 5]) -> u8 {
    let t = temp as f32;
    let p0 = curve[0] as f32;
    let p1 = curve[1] as f32;
    let p2 = curve[2] as f32;
    let p3 = curve[3] as f32;
    let p4 = curve[4] as f32;

    let target_pct = if t <= 50.0 {
        p0
    } else if t <= 60.0 {
        p0 + (t - 50.0) / 10.0 * (p1 - p0)
    } else if t <= 70.0 {
        p1 + (t - 60.0) / 10.0 * (p2 - p1)
    } else if t <= 80.0 {
        p2 + (t - 70.0) / 10.0 * (p3 - p2)
    } else if t <= 90.0 {
        p3 + (t - 80.0) / 10.0 * (p4 - p3)
    } else {
        p4
    };

    target_pct.round().clamp(0.0, 100.0) as u8
}

fn pct_to_nearest_pwm(pct: u8) -> u8 {
    if pct < 20 {
        return 0; // Silent stop
    }
    let mut closest_pwm = PWM_TABLE[0].1;
    let mut min_diff = i16::MAX;

    for &(step_pct, pwm) in &PWM_TABLE {
        let diff = (step_pct as i16 - pct as i16).abs();
        if diff < min_diff {
            min_diff = diff;
            closest_pwm = pwm;
        }
    }

    closest_pwm
}

fn main() -> Result<(), slint::PlatformError> {
    let main_window = AppWindow::new()?;

    // 1. Load persisted configuration
    let config = Arc::new(Mutex::new(AppConfig::load()));

    // 2. Initialize HAL controllers
    let (thermal_ctrl, lighting_ctrl) = create_controllers();
    let thermal_controller = Arc::new(Mutex::new(thermal_ctrl));
    let lighting_controller = Arc::new(Mutex::new(lighting_ctrl));
    let last_applied_custom_target = Arc::new(Mutex::new(None::<u8>));

    // 3. Apply persisted configuration to hardware and UI immediately on startup
    {
        let mut cfg = config.lock();

        let mode_str = match cfg.fan_mode {
            FanMode::Quiet => "Quiet",
            FanMode::Gaming => "Gaming",
            FanMode::Turbo => "Turbo",
            FanMode::Custom => "Custom",
            _ => "Normal",
        };
        main_window.set_active_mode(mode_str.into());

        // Set 5-point curve values in UI
        main_window.set_curve_50(cfg.custom_curve[0] as f32);
        main_window.set_curve_60(cfg.custom_curve[1] as f32);
        main_window.set_curve_70(cfg.custom_curve[2] as f32);
        main_window.set_curve_80(cfg.custom_curve[3] as f32);
        main_window.set_curve_90(cfg.custom_curve[4] as f32);

        main_window.set_kb_brightness(cfg.kb_brightness as f32);
        main_window.set_active_kb_color(cfg.kb_color.to_name().into());
        main_window.set_active_bar_color(cfg.bar_color.to_name().into());
        main_window.set_lightbar_enabled(cfg.bar_enabled);
        main_window.set_battery_limit(cfg.battery_limit);

        // Apply fan and lighting to hardware via HAL
        let mut tc = thermal_controller.lock();
        let _ = tc.set_fan_mode(cfg.fan_mode);
        let _ = tc.set_battery_charge_limit(cfg.battery_limit);

        // Read REAL hardware MUX state on startup
        if let Ok(real_gpu_mode) = tc.get_gpu_mode() {
            main_window.set_active_gpu_mode(real_gpu_mode.to_name().into());
            cfg.gpu_mode = real_gpu_mode;
            let _ = cfg.save();
        } else {
            main_window.set_active_gpu_mode(cfg.gpu_mode.to_name().into());
        }

        // Read REAL dGPU hardware status on startup
        if let Ok(is_active) = tc.get_dgpu_status() {
            let is_killed = !is_active;
            main_window.set_dgpu_killed(is_killed);
            cfg.dgpu_killed = is_killed;
            let _ = cfg.save();
        } else {
            main_window.set_dgpu_killed(cfg.dgpu_killed);
        }

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

    // 5. Keyboard Brightness Live Drag Callback
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

    // 6. Keyboard Brightness Save Callback
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
        let custom_target_clone = last_applied_custom_target.clone();

        main_window.on_set_fan_mode(move |mode_str| {
            let mode = match mode_str.as_str() {
                "Quiet" => FanMode::Quiet,
                "Gaming" => FanMode::Gaming,
                "Turbo" => FanMode::Turbo,
                "Custom" => FanMode::Custom,
                _ => FanMode::Normal,
            };

            *custom_target_clone.lock() = None;

            {
                let mut cfg = cfg_clone.lock();
                cfg.fan_mode = mode;
                let _ = cfg.save();
            }

            let mut tc = tc_clone.lock();
            let _ = tc.set_fan_mode(mode);
        });
    }

    // 10. Custom Curve Point Change Callback
    {
        let cfg_clone = config.clone();

        main_window.on_set_curve_point(move |idx, val| {
            if idx >= 0 && idx < 5 {
                let mut cfg = cfg_clone.lock();
                cfg.custom_curve[idx as usize] = (val as u8).clamp(0, 100);
            }
        });
    }

    // 11. Save Custom Curve Callback (on slider release)
    {
        let cfg_clone = config.clone();
        let custom_target_clone = last_applied_custom_target.clone();

        main_window.on_save_curve(move || {
            *custom_target_clone.lock() = None;
            let cfg = cfg_clone.lock();
            let _ = cfg.save();
        });
    }

    // 12. MUX Switch Callback (Fixed regression: updates active_gpu_mode immediately)
    {
        let tc_clone = thermal_controller.clone();
        let cfg_clone = config.clone();
        let window_weak = main_window.as_weak();

        main_window.on_set_gpu_mode(move |mode_str| {
            let mode = GpuMode::from_name(mode_str.as_str());

            // If switching to Discrete while dGPU was killed, auto-enable dGPU first
            if mode == GpuMode::Discrete {
                let mut tc = tc_clone.lock();
                let _ = tc.set_dgpu_enabled(true);
                if let Some(w) = window_weak.upgrade() {
                    w.set_dgpu_killed(false);
                }
                let mut cfg = cfg_clone.lock();
                cfg.dgpu_killed = false;
                cfg.gpu_mode = mode;
                let _ = cfg.save();
                let _ = tc.set_gpu_mode(mode);
            } else {
                let mut cfg = cfg_clone.lock();
                cfg.gpu_mode = mode;
                let _ = cfg.save();
                let mut tc = tc_clone.lock();
                let _ = tc.set_gpu_mode(mode);
            }

            // Update UI property so button highlight updates immediately!
            if let Some(w) = window_weak.upgrade() {
                w.set_active_gpu_mode(mode.to_name().into());
                w.set_dgpu_notice("⚠️ MUX modu değiştirildi. Donanım seviyesinde etkili olması için sistemi yeniden başlatın.".into());
            }
        });
    }

    // 13. dGPU Kill-Switch Callback (with pending-reboot & hardware safety lock)
    {
        let tc_clone = thermal_controller.clone();
        let cfg_clone = config.clone();
        let window_weak = main_window.as_weak();

        main_window.on_toggle_dgpu_kill(move || {
            // Check real physical hardware MUX mode
            let real_hw_mode = {
                let tc = tc_clone.lock();
                tc.get_gpu_mode().unwrap_or(GpuMode::Discrete)
            };

            let (cfg_mode, is_killed) = {
                let cfg = cfg_clone.lock();
                (cfg.gpu_mode, cfg.dgpu_killed)
            };

            // PENDING REBOOT / DISCRETE PROTECTION:
            // If physical hardware is Discrete, OR config is Discrete:
            // Siyah ekran riski var! Kapatılamaz!
            if real_hw_mode == GpuMode::Discrete || cfg_mode == GpuMode::Discrete {
                if let Some(w) = window_weak.upgrade() {
                    w.set_active_gpu_mode("Hybrid".into());
                    w.set_dgpu_notice("⚠️ Donanım Discrete modda (siyah ekran riski)! MUX Hybrid moda alındı, dGPU kapatılmadan önce MUTLAKA bilgisayar yeniden başlatılmalıdır.".into());
                }

                let mut tc = tc_clone.lock();
                let _ = tc.set_gpu_mode(GpuMode::Hybrid);
                let mut cfg = cfg_clone.lock();
                cfg.gpu_mode = GpuMode::Hybrid;
                let _ = cfg.save();
                return;
            }

            // In confirmed pure Hybrid mode: toggle dGPU state safely
            let mut tc = tc_clone.lock();
            if is_killed {
                let _ = tc.set_dgpu_enabled(true);
                if let Some(w) = window_weak.upgrade() {
                    w.set_dgpu_killed(false);
                    w.set_dgpu_notice("✅ RTX 4060 başarıyla açıldı.".into());
                }
                let mut cfg = cfg_clone.lock();
                cfg.dgpu_killed = false;
                let _ = cfg.save();
            } else {
                let _ = tc.set_dgpu_enabled(false);
                if let Some(w) = window_weak.upgrade() {
                    w.set_dgpu_killed(true);
                    w.set_dgpu_notice("✅ RTX 4060 kapatıldı. Maksimum pil tasarrufu devrede (1-2 sn kırpışma normaldir).".into());
                }
                let mut cfg = cfg_clone.lock();
                cfg.dgpu_killed = true;
                let _ = cfg.save();
            }
        });
    }

    // 14. Battery Limit Callback
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

    // 15. Non-blocking Background Telemetry & Custom Fan Curve Loop
    {
        let window_weak = main_window.as_weak();
        let cfg_clone = config.clone();
        let custom_target_clone = last_applied_custom_target.clone();

        std::thread::spawn(move || {
            loop {
                if let Ok(telemetry) = hal::windows::fetch_telemetry() {
                    let weak = window_weak.clone();
                    let _ = slint::invoke_from_event_loop(move || {
                        if let Some(window) = weak.upgrade() {
                            window.set_cpu_temp(format!("{} \u{00B0}C", telemetry.cpu_temp).into());
                            window.set_cpu_usage(format!("{}%", telemetry.cpu_usage).into());
                            window.set_gpu_temp(format!("{} \u{00B0}C", telemetry.gpu_temp).into());
                            window.set_gpu_usage(format!("{}%", telemetry.gpu_usage).into());
                            window.set_fan1_rpm(format!("{} RPM", telemetry.fan1_rpm).into());
                            window.set_fan2_rpm(format!("{} RPM", telemetry.fan2_rpm).into());
                        }
                    });

                    // Custom Fan Curve Engine with Hysteresis
                    let (mode, curve) = {
                        let cfg = cfg_clone.lock();
                        (cfg.fan_mode, cfg.custom_curve)
                    };

                    if mode == FanMode::Custom {
                        let max_temp = telemetry.cpu_temp.max(telemetry.gpu_temp);
                        let target_pct = interpolate_curve(max_temp, &curve);
                        let mut target_lock = custom_target_clone.lock();

                        let should_apply = match *target_lock {
                            None => true,
                            Some(last) => (target_pct as i16 - last as i16).abs() >= 5,
                        };

                        if should_apply {
                            *target_lock = Some(target_pct);
                            let pwm = pct_to_nearest_pwm(target_pct);
                            hal::windows::apply_pwm_speed(pwm);
                        }
                    }
                }

                std::thread::sleep(Duration::from_millis(2000));
            }
        });
    }

    let run_res = main_window.run();

    // Restore automatic fan control on application exit
    hal::windows::restore_auto_fans();

    run_res
}
