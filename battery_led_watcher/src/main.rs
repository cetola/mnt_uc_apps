use notify::{Watcher, RecursiveMode, Event, RecommendedWatcher, Config};
use std::env;
use std::io::{self, Write};
use std::path::Path;
use std::time::Duration;
use tokio::sync::mpsc;
use tokio::time::timeout;
use tokio::{fs, task};

/// Paths to watch
const STATUS_PATH: &str = "/sys/class/power_supply/BAT0/status";
const CAPACITY_PATH: &str = "/sys/class/power_supply/BAT0/capacity";
const SERIAL_PORT: &str = "/dev/ttyACM1";

#[tokio::main(flavor = "current_thread")]
async fn main() -> io::Result<()> {
    // 1) Parse intensity arg
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

    // 2) Channel to receive file events
    let (tx, mut rx) = mpsc::channel(10);

    // Clone `tx` so the closure gets its own handle
    let tx_watcher = tx.clone();
    let mut watcher: RecommendedWatcher = Watcher::new(
        move |res: Result<Event, notify::Error>| {
            if let Ok(_ev) = res {
                let _ = tx_watcher.try_send(());  // uses clone, not original
            }
        },
        Config::default().with_poll_interval(Duration::from_secs(1)),
    ).unwrap();

    watcher
        .watch(Path::new(STATUS_PATH), RecursiveMode::NonRecursive)
        .unwrap();
    watcher
        .watch(Path::new(CAPACITY_PATH), RecursiveMode::NonRecursive)
        .unwrap();

    // 4) On startup, trigger one check
    let _ = tx.try_send(());

    // 5) Process events
    while let Some(_) = rx.recv().await {
        // Debounce: wait up to 100 ms for further changes
        let _ = timeout(Duration::from_millis(100), rx.recv()).await;

        // Read current status & capacity
        let status = read_trimmed(STATUS_PATH).await.unwrap_or_default();
        let capacity = read_trimmed(CAPACITY_PATH)
            .await
            .ok()
            .and_then(|s| s.parse::<u8>().ok())
            .unwrap_or(0);

        // Decide LED color
        let color = match status.as_str() {
            "Charging" if capacity < 100 => "blue",
            "Charging" | "Full"     => "green",
            "Discharging"           => "red",
            _                       => continue, // unknown state
        };

        // Send to serial port
        if let Err(e) = send_led_command(color, intensity).await {
            eprintln!("Failed to send LED command: {}", e);
        }
    }

    Ok(())
}

/// Read a small text file and trim whitespace.
async fn read_trimmed(path: &str) -> io::Result<String> {
    let mut data = fs::read(path).await?;
    // Remove trailing newline
    if let Some(&b'\n') = data.last() {
        data.pop();
    }
    String::from_utf8(data)
        .map_err(|e| io::Error::new(io::ErrorKind::InvalidData, e))
}

/// Write the LED command to the serial port.
/// Opens, writes, and closes for simplicity; you could keep it open if you prefer.
async fn send_led_command(color: &str, intensity: u8) -> io::Result<()> {
    let cmd = format!("setled {} {}\n", color, intensity);
    // Offload blocking I/O to a thread-pool
    task::spawn_blocking(move || {
        let mut port = std::fs::OpenOptions::new()
            .write(true)
            .open(SERIAL_PORT)?;
        port.write_all(cmd.as_bytes())
    })
    .await?
}
