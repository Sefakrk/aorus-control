#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod hardware;
mod rgb;

slint::include_modules!();

fn preset_to_color_idx(preset: &str) -> u8 {
    match preset {
        "Red" => 1,
        "Green" => 2,
        "Yellow" => 3,
        "Blue" => 4,
        "Orange" => 5,
        "Purple" => 6,
        "White" => 7,
        "Off" => 0,
        _ => 4, // Default to Blue
    }
}

fn main() -> Result<(), slint::PlatformError> {
    let main_window = AppWindow::new()?;

    // State trackers
    let kb_idx = std::sync::Arc::new(std::sync::atomic::AtomicU8::new(4));
    let kb_brightness = std::sync::Arc::new(std::sync::atomic::AtomicU8::new(100));

    let bar_idx = std::sync::Arc::new(std::sync::atomic::AtomicU8::new(4));
    let bar_enabled = std::sync::Arc::new(std::sync::atomic::AtomicBool::new(true));

    // Keyboard Color Callback
    {
        let idx_c = kb_idx.clone();
        let br_c = kb_brightness.clone();

        main_window.on_set_keyboard_color(move |preset| {
            let idx = preset_to_color_idx(preset.as_str());
            idx_c.store(idx, std::sync::atomic::Ordering::Relaxed);

            if idx == 0 {
                rgb::turn_keyboard_off();
            } else {
                let br = br_c.load(std::sync::atomic::Ordering::Relaxed);
                rgb::set_keyboard_color(br, idx);
            }
        });
    }

    // Keyboard Brightness Callback
    {
        let idx_c = kb_idx.clone();
        let br_c = kb_brightness.clone();

        main_window.on_set_keyboard_brightness(move |val| {
            let br = val.clamp(0, 100) as u8;
            br_c.store(br, std::sync::atomic::Ordering::Relaxed);

            let idx = idx_c.load(std::sync::atomic::Ordering::Relaxed);
            if br == 0 || idx == 0 {
                rgb::turn_keyboard_off();
            } else {
                rgb::set_keyboard_color(br, idx);
            }
        });
    }

    // Lightbar Color Callback
    {
        let idx_c = bar_idx.clone();
        let en_c = bar_enabled.clone();

        main_window.on_set_lightbar_color(move |preset| {
            let idx = preset_to_color_idx(preset.as_str());
            idx_c.store(idx, std::sync::atomic::Ordering::Relaxed);

            let enabled = en_c.load(std::sync::atomic::Ordering::Relaxed);
            if idx == 0 || !enabled {
                rgb::turn_lightbar_off();
            } else {
                rgb::set_lightbar_color(true, idx);
            }
        });
    }

    // Lightbar Toggle Callback
    {
        let idx_c = bar_idx.clone();
        let en_c = bar_enabled.clone();

        main_window.on_toggle_lightbar(move |enabled| {
            en_c.store(enabled, std::sync::atomic::Ordering::Relaxed);

            let idx = idx_c.load(std::sync::atomic::Ordering::Relaxed);
            if enabled && idx > 0 {
                rgb::set_lightbar_color(true, idx);
            } else {
                rgb::turn_lightbar_off();
            }
        });
    }

    // Fan Mode Callback
    main_window.on_set_fan_mode(|mode| {
        hardware::set_fan_mode(mode.as_str());
    });

    // Custom Fan Speed Callback
    main_window.on_set_custom_fan(|speed| {
        hardware::set_custom_fan_speed(speed.clamp(30, 100) as u8);
    });

    // Battery Limit Callback
    main_window.on_toggle_battery_limit(|enabled| {
        hardware::set_battery_limit(enabled);
    });

    // Periodic sensor update timer (every 2 seconds)
    let window_handle = main_window.as_weak();
    let timer = slint::Timer::default();
    timer.start(
        slint::TimerMode::Repeated,
        std::time::Duration::from_millis(2000),
        move || {
            if let Some(window) = window_handle.upgrade() {
                let (cpu, gpu, rpm) = hardware::read_sensors();
                window.set_cpu_temp(cpu.into());
                window.set_gpu_temp(gpu.into());
                window.set_fan_rpm(rpm.into());
            }
        },
    );

    // Initial sensor read
    let (cpu, gpu, rpm) = hardware::read_sensors();
    main_window.set_cpu_temp(cpu.into());
    main_window.set_gpu_temp(gpu.into());
    main_window.set_fan_rpm(rpm.into());

    main_window.run()
}
