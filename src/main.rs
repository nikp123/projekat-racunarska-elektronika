// Prevent console window in addition to Slint window in Windows release builds when, e.g., starting the app via file manager. Ignored on other platforms.
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use std::error::Error;
use std::io;
use std::sync::{
    atomic::{AtomicBool, Ordering},
    Arc,
    Mutex
};
use std::thread;
use std::time::Duration;
use gpiod::{Chip, Lines, Options, Output};

use adb_client::{ADBDeviceExt, ADBServer, ADBServerDevice};

slint::include_modules!();

fn startup() -> Result<Lines<Output>, String> {
    // assume gpiochip0 because I cant be bothered to do even more UI for this
    let chip = match Chip::new(0) {
        Ok(chip) => chip,
        Err(_) => return Err("Unable to use gpiochip0! Do you have the right permissions?".to_string())
    };

    let opts = Options::output([27, 22, 23, 24])
        .values([false, false, false, false])
        .consumer("rotating-table");

    let output = match chip.request_lines(opts) {
        Ok(output) => output,
        Err(_) => return Err("Unable to configure outputs. Are you **not** on a Pi?".to_string())
    };

    Ok(output)
}

fn scan_adb_devices(ui: &AppWindow) -> Option<ADBServerDevice> {
    let mut server = ADBServer::default();
    let devices = match server.devices() {
        Ok(thing) => thing,
        Err(_) => return None
    };

    if devices.len() == 0 {
        return None;
    }

    // will select the first device because I can't be bothered
    // to implement a multidevice dialog
    let device_name = devices[0].identifier.clone();

    let device = match server.get_device_by_name(&device_name) {
        Ok(d) => d,
        Err(_) => return None
    };
    
    ui.set_can_proceed(true);
    ui.set_active_tab(1);
    ui.set_device_name((&device_name).into());

    #[cfg(debug_assertions)] {
        dbg!(devices);
    }

    Some(device)
}

fn open_camera_app(device: &mut Option<ADBServerDevice>) {
    // discard output
    let mut nowhere = io::sink();

   if let Some(device) = device.as_mut() {
        let _ = device.shell_command(
            &["am", "start", "-a", "android.media.action.IMAGE_CAPTURE"],
            &mut nowhere);
    } 
    // The UI should be blocking you from being here
}

fn main() -> Result<(), Box<dyn Error>> {
    let gpio = match startup() {
        Ok(d) => Arc::new(Mutex::new(d)),
        Err(e) => {
            let ui = ErrorDialog::new()?;

            let message = e;

            ui.set_error_text(message.into());
            ui.on_close_clicked({
                let ui_handle = ui.as_weak();
                move || {
                    let ui = ui_handle.unwrap();
                    let _ = ui.hide();
                }
            });
            ui.run()?;

            return Ok(());
        }
    };

    let ui = Arc::new(AppWindow::new()?);
    let device = Arc::new(Mutex::new(scan_adb_devices(&ui)));

    // Update ADB device list action
    {
        let ui_handle = ui.as_weak();

        let device = Arc::clone(&device);

        ui.on_scan_adb_devices(move || {
            let ui = ui_handle.unwrap();
            let mut d = device.lock().unwrap();
            *d = scan_adb_devices(&ui);
        });
    }

    // Open camera action
    {
        let device = Arc::clone(&device);

        ui.on_open_camera_app(move || {
            let mut d = device.lock().unwrap();
            open_camera_app(&mut d);
        });
    }

    // Start capture action + thread logic
    let stop_flag = Arc::new(AtomicBool::new(false));
    {
        let ui_handle = ui.as_weak();

        let device = Arc::clone(&device);
        let flag = Arc::clone(&stop_flag);
        let gpio = Arc::clone(&gpio);

        ui.on_start_capture(move || {
            flag.store(false, Ordering::Relaxed);

            let ui = ui_handle.clone().upgrade().unwrap();

            let steps = ui.get_steps() * ui.get_revolutions();
            let step_size = 4096.0 / ui.get_steps() as f64;

            let camera_delay = ui.get_delay();          // in seconds
            let step_delay   = ui.get_delay_step();     // in millis

            let ui_handle_clone = ui_handle.clone();
            let flag_clone = flag.clone();
            let device_clone = device.clone();
            let gpio_clone = gpio.clone();

            // Your thread handle, in case you need to kill or join it
            let _ = thread::spawn(move || {
                #[cfg(debug_assertions)]
                {
                    println!("Started rotation thread with {} steps and {} step_size",
                        steps, step_size);
                }

                // Stepper motor driving pattern so that
                // the coils get powered on in order
                let motor_pattern: [[bool; 4]; 8] = [
                    [true,false,false,true],
                    [true,false,false,false],
                    [true,true,false,false],
                    [false,true,false,false],
                    [false,true,true,false],
                    [false,false,true,false],
                    [false,false,true,true],
                    [false,false,false,true]];

                for i in 0..steps {
                    if flag_clone.load(Ordering::Relaxed) {
                        break;
                    }

                    let mut nowhere = io::sink();

                    if let Some(device) = device_clone.lock().unwrap().as_mut() {
                        let _ = device.shell_command(
                            &["input", "keyevent", "KEYCODE_CAMERA"],
                            &mut nowhere);
                    } 

                    // drive motors
                    let counter = i as f64;
                    let begin_step = (step_size * counter).round() as usize;
                    let end_step = (step_size * counter).round() as usize;
                    for j in begin_step..end_step {
                        // Move the motor by a step
                        gpio_clone.lock().unwrap().set_values(motor_pattern[j%8]).unwrap();
                        // Wait for the motor to settle in position
                        thread::sleep(Duration::from_millis(step_delay as u64));
                    }
                    
                    let mut wait_between_camera = (camera_delay * 1000.0) as i32;
                    wait_between_camera -= step_delay * ((end_step-begin_step) as i32);

                    // sometimes the motor may be holding up the camera
                    if wait_between_camera > 0 {
                        thread::sleep(Duration::from_millis(wait_between_camera as u64));
                    }

                    let ui_handle_clone_clone = ui_handle_clone.clone();

                    // Update capture progress
                    let _ = slint::invoke_from_event_loop(move || {
                        if let Some(ui) = ui_handle_clone_clone.upgrade() {
                            let mut progress = i as f32;
                            progress /= steps as f32;

                            ui.set_capture_progress(progress);
                        }
                    });
                }

                // Clean up the UI once we're done with the capture
                let ui_handle_clone_clone = ui_handle_clone.clone();
                let _ = slint::invoke_from_event_loop(move || {
                    if let Some(ui) = ui_handle_clone_clone.upgrade() {
                        ui.set_capture_in_progress(false);
                        ui.set_capture_progress(0.0);
                    }
                });

                #[cfg(debug_assertions)]
                {
                    println!("Stopped thread");
                }
            });
        });
    }

    // Stop capture action
    {
        let flag = Arc::clone(&stop_flag);

        ui.on_stop_capture(move || {            
            flag.store(true, Ordering::Relaxed);
        });
    }
    ui.run()?;

    Ok(())
}
