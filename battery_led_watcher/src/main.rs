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
use std::time::Duration;
use tokio::task;
use tokio::time::sleep;

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

            if let Err(e) = send_led_command(&color, intensity).await {
                eprintln!("Failed to send LED command: {}", e);
            }
        }

        sleep(Duration::from_secs(1)).await;
    }
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

async fn send_led_command(color: &str, intensity: u8) -> io::Result<()> {
    let cmd = format!("setled {} {}\n", color, intensity);
    let cmd_bytes = cmd.into_bytes();
    println!("Set LED to : {}", color);
    task::spawn_blocking(move || {
        let mut port = fs::OpenOptions::new()
            .write(true)
            .open(SERIAL_PORT)?;
        port.write_all(&cmd_bytes)
    })
    .await?
}

