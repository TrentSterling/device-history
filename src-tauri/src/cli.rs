use crate::logging::log_to_file;
use crate::types::UsbDevice;
use chrono::Local;
use colored::*;
use std::collections::HashMap;
use std::thread;
use std::time::Duration;
use wmi::{COMLibrary, WMIConnection};

fn query_devices(wmi: &WMIConnection) -> Option<HashMap<String, UsbDevice>> {
    let results: Vec<UsbDevice> = wmi
        .raw_query(
            "SELECT Name, DeviceID, Description, Manufacturer, PNPClass \
             FROM Win32_PnPEntity WHERE DeviceID LIKE 'USB%'",
        )
        .ok()?;
    Some(
        results
            .into_iter()
            .filter_map(|d| Some((d.DeviceID.clone()?, d)))
            .collect(),
    )
}

pub fn run_cli() {
    #[cfg(windows)]
    unsafe {
        extern "system" {
            fn AttachConsole(dwProcessId: u32) -> i32;
            fn AllocConsole() -> i32;
        }
        if AttachConsole(0xFFFFFFFF) == 0 {
            AllocConsole();
        }
    }

    let ver = env!("CARGO_PKG_VERSION");
    let title = format!("Device History v{}", ver);
    let tagline = "WTF just disconnected?";
    let width = 39;
    println!(
        "{}",
        format!("\u{2554}{}\u{2557}", "\u{2550}".repeat(width)).bright_cyan()
    );
    println!(
        "{}",
        format!("\u{2551}{:^w$}\u{2551}", title, w = width).bright_cyan()
    );
    println!(
        "{}",
        format!("\u{2551}{:^w$}\u{2551}", tagline, w = width).bright_cyan()
    );
    println!(
        "{}",
        format!("\u{255a}{}\u{255d}", "\u{2550}".repeat(width)).bright_cyan()
    );
    println!();

    let com = COMLibrary::new().expect("Failed to initialize COM library");
    let wmi = WMIConnection::new(com).expect("Failed to connect to WMI");
    let mut devices = query_devices(&wmi).expect("Failed to query USB devices");

    println!(
        "{} {} USB devices currently connected:\n",
        "*".green(),
        devices.len().to_string().bold()
    );

    let mut sorted: Vec<_> = devices.values().collect();
    sorted.sort_by_key(|d| d.display_name().to_lowercase());
    for dev in &sorted {
        let vid_pid = dev
            .vid_pid()
            .map(|vp| format!(" [{}]", vp))
            .unwrap_or_default();
        let mfr = dev
            .Manufacturer
            .as_deref()
            .map(|m| format!(" ({})", m))
            .unwrap_or_default();
        println!(
            "  {} {} {}{}{}",
            "|".dimmed(),
            dev.class().dimmed(),
            dev.display_name(),
            vid_pid.dimmed(),
            mfr.dimmed()
        );
    }

    log_to_file(&format!(
        "Started monitoring (CLI) — {} devices",
        devices.len()
    ));
    println!("\n{}", "Watching for changes... (Ctrl+C to quit)".dimmed());
    println!("{}\n", "\u{2500}".repeat(60).dimmed());

    loop {
        thread::sleep(Duration::from_millis(500));
        let Some(current) = query_devices(&wmi) else {
            continue;
        };

        for (id, dev) in &devices {
            if !current.contains_key(id) {
                let ts = Local::now().format("%H:%M:%S").to_string();
                let vp = dev
                    .vid_pid()
                    .map(|v| format!(" [{}]", v))
                    .unwrap_or_default();
                println!(
                    "{} {} {} {}",
                    format!("[{}]", ts).dimmed(),
                    "\u{25BC} DISCONNECT".red().bold(),
                    dev.display_name().red(),
                    vp.yellow()
                );
                log_to_file(&format!(
                    "DISCONNECT: {} {} | {}",
                    dev.display_name(),
                    vp,
                    id
                ));
            }
        }
        for (id, dev) in &current {
            if !devices.contains_key(id) {
                let ts = Local::now().format("%H:%M:%S").to_string();
                let vp = dev
                    .vid_pid()
                    .map(|v| format!(" [{}]", v))
                    .unwrap_or_default();
                println!(
                    "{} {} {} {}",
                    format!("[{}]", ts).dimmed(),
                    "\u{25B2} CONNECT   ".green().bold(),
                    dev.display_name().green(),
                    vp.yellow()
                );
                log_to_file(&format!(
                    "CONNECT: {} {} | {}",
                    dev.display_name(),
                    vp,
                    id
                ));
            }
        }
        devices = current;
    }
}
