/*
 * Copyright (c) 2025 Stephano Cetola
 *
 * SPDX-License-Identifier: Apache-2.0
 */

const STATUS_PATH: &str = "/sys/class/power_supply/BAT0/status";
const CAPACITY_PATH: &str = "/sys/class/power_supply/BAT0/capacity";
const SERIAL_PORT: &str = "/dev/ttyACM1";

use std::io::{self, Write};
use std::fs;
use std::env;
use std::sync::{Arc, Mutex};
use std::time::Duration;
use tokio::task;
use tokio::time::sleep;
use serialport::SerialPort;

#[tokio::main]
async fn main() -> io::Result<()> {
    //Parse intensity
    let intensity: u8 = match env::args().nth(1) {
        Some(s) => match s.parse::<u8>() {
            Ok(n @ 1..=255) => n,
            _ => {
                eprintln!("Error: Intensity must be a number between 1 and 255.");
                std::process::exit(1);
            }
        },
        None => 255,
    };

    // Open and configure serial port once
    let mut port = serialport::new(SERIAL_PORT, 115200)
        .timeout(Duration::from_millis(100))
        .open()
        .map_err(|e| io::Error::new(io::ErrorKind::Other, e))?;

    // Set to raw mode and assert DTR/RTS
    configure_serial_port(&mut port)?;

    // Wrap port in Arc<Mutex<>> for safe shared access
    // This ensures only one write happens at a time
    let port = Arc::new(Mutex::new(port));

    let mut last_status = String::new();
    let mut last_capacity = 0;

    loop {
        let status = read_trimmed(STATUS_PATH).await.unwrap_or_default();
        let capacity = read_trimmed(CAPACITY_PATH)
            .await
            .ok()
            .and_then(|s| s.parse::<u8>().ok())
            .unwrap_or(0);

        //Quirks: the microcontroller sometimes reports 0 for the capacity
        //which is very much not true and should be ignored.
        //Also, "Not charging" can be ignored. Assume it's full.
        if capacity != 0 && status != "Not charging" &&
            (status != last_status || capacity != last_capacity) {
            last_status = status.clone();
            last_capacity = capacity;

            let color = match status.as_str() {
                "Charging" if capacity < 100 => "blue",
                "Charging" | "Full"          => "green",
                "Discharging"                => "red",
                _                            => continue,
            };

            if let Err(e) = send_led_command(port.clone(), &color, intensity).await {
                eprintln!("Failed to send LED command: {}", e);
            }
        }

        sleep(Duration::from_secs(1)).await;
    }
}

fn configure_serial_port(port: &mut Box<dyn SerialPort>) -> io::Result<()> {
    // Assert DTR and RTS
    port.write_data_terminal_ready(true)
        .map_err(|e| io::Error::new(io::ErrorKind::Other, e))?;
    port.write_request_to_send(true)
        .map_err(|e| io::Error::new(io::ErrorKind::Other, e))?;
    
    Ok(())
}

async fn read_trimmed(path: &str) -> io::Result<String> {
    let path = path.to_string();
    task::spawn_blocking(move || {
        let mut data = fs::read(path)?;
        // Remove trailing newline
        if let Some(&b'\n') = data.last() {
            data.pop();
        }
        String::from_utf8(data)
            .map_err(|e| io::Error::new(io::ErrorKind::InvalidData, e))
    })
    .await?
}

async fn send_led_command(port: Arc<Mutex<Box<dyn SerialPort>>>, color: &str, intensity: u8) -> io::Result<()> {
    let cmd = format!("setled {} {}\n", color, intensity);
    println!("Set LED to : {}", color);
    let _ = std::fs::write("/tmp/battery_led_color", color);
    
    // Serial port operations are blocking I/O. Run them in spawn_blocking
    // to avoid blocking the async runtime, but ensure they complete sequentially.
    let port_clone = port.clone();
    let cmd_bytes = cmd.into_bytes();
    
    task::spawn_blocking(move || {
        let mut port_guard = port_clone.lock().unwrap();
        port_guard.write_all(&cmd_bytes)
            .map_err(|e| io::Error::new(io::ErrorKind::Other, e))?;
        port_guard.flush()
            .map_err(|e| io::Error::new(io::ErrorKind::Other, e))?;
        // Add delay here in the blocking context to ensure transmission completes
        std::thread::sleep(Duration::from_millis(150));
        Ok::<(), io::Error>(())
    })
    .await??;
    
    Ok(())
}

