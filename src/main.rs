// Hide console window in GUI mode (release builds only)
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use chrono::Local;
use eframe::egui;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fs::OpenOptions;
use std::io::Write as IoWrite;
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::{Duration, Instant};
use tray_icon::menu::{Menu, MenuEvent, MenuItem, PredefinedMenuItem};
use tray_icon::{TrayIconBuilder, TrayIconEvent};
use wmi::{COMLibrary, WMIConnection};

// ── Win32 FFI for window show/hide ──────────────────────────────

#[cfg(windows)]
mod win32 {
    use std::sync::atomic::{AtomicIsize, Ordering};

    extern "system" {
        pub fn ShowWindow(hWnd: isize, nCmdShow: i32) -> i32;
        pub fn FindWindowW(lpClassName: *const u16, lpWindowName: *const u16) -> isize;
        pub fn SetForegroundWindow(hWnd: isize) -> i32;
        pub fn BringWindowToTop(hWnd: isize) -> i32;
        pub fn IsWindow(hWnd: isize) -> i32;
    }

    pub const SW_HIDE: i32 = 0;
    pub const SW_SHOW: i32 = 5;
    pub const SW_RESTORE: i32 = 9;

    static CACHED_HWND: AtomicIsize = AtomicIsize::new(0);

    pub fn find_hwnd() -> Option<isize> {
        // Try cached handle first
        let cached = CACHED_HWND.load(Ordering::Relaxed);
        if cached != 0 && unsafe { IsWindow(cached) } != 0 {
            return Some(cached);
        }

        let title: Vec<u16> = "Device History\0".encode_utf16().collect();
        let hwnd = unsafe { FindWindowW(std::ptr::null(), title.as_ptr()) };
        if hwnd != 0 {
            CACHED_HWND.store(hwnd, Ordering::Relaxed);
            Some(hwnd)
        } else {
            None
        }
    }

    pub fn show_window() {
        if let Some(hwnd) = find_hwnd() {
            unsafe {
                ShowWindow(hwnd, SW_SHOW);
                ShowWindow(hwnd, SW_RESTORE);
                BringWindowToTop(hwnd);
                SetForegroundWindow(hwnd);
            }
        }
    }

    pub fn hide_window() {
        if let Some(hwnd) = find_hwnd() {
            unsafe {
                ShowWindow(hwnd, SW_HIDE);
            }
        }
    }
}

// ── WMI device struct ──────────────────────────────────────────

#[derive(Deserialize, Debug, Clone)]
#[allow(non_snake_case)]
struct UsbDevice {
    Name: Option<String>,
    DeviceID: Option<String>,
    Description: Option<String>,
    Manufacturer: Option<String>,
    PNPClass: Option<String>,
}

impl UsbDevice {
    fn display_name(&self) -> &str {
        self.Name
            .as_deref()
            .or(self.Description.as_deref())
            .unwrap_or("Unknown Device")
    }

    fn vid_pid(&self) -> Option<String> {
        let id = self.DeviceID.as_ref()?;
        let vid_start = id.find("VID_").map(|i| i + 4)?;
        let vid = id.get(vid_start..vid_start + 4)?;
        let pid_start = id.find("PID_").map(|i| i + 4)?;
        let pid = id.get(pid_start..pid_start + 4)?;
        Some(format!("{}:{}", vid, pid))
    }

    fn class(&self) -> &str {
        self.PNPClass.as_deref().unwrap_or("?")
    }
}

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

// ── WMI Storage Queries ──────────────────────────────────────────

#[derive(Deserialize, Debug, Clone)]
#[allow(non_snake_case)]
struct WmiDiskDrive {
    DeviceID: Option<String>,
    PNPDeviceID: Option<String>,
    Model: Option<String>,
    SerialNumber: Option<String>,
    Size: Option<u64>,
    InterfaceType: Option<String>,
    MediaType: Option<String>,
    Partitions: Option<u32>,
    FirmwareRevision: Option<String>,
    Status: Option<String>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
struct StorageInfo {
    model: String,
    serial_number: String,
    total_bytes: u64,
    interface_type: String,
    media_type: String,
    firmware: String,
    partition_count: u32,
    status: String,
    volumes: Vec<VolumeInfo>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
struct VolumeInfo {
    drive_letter: String,
    volume_name: String,
    total_bytes: u64,
    free_bytes: u64,
    file_system: String,
    volume_serial: String,
}

fn is_storage_device(dev: &UsbDevice) -> bool {
    let class = dev.PNPClass.as_deref().unwrap_or("");
    let name = dev.Name.as_deref().unwrap_or("");
    class.contains("SCSIAdapter")
        || class.contains("DiskDrive")
        || (class.contains("USB") && name.contains("Storage"))
        || name.contains("Mass Storage")
}

fn format_bytes(bytes: u64) -> String {
    const KB: u64 = 1024;
    const MB: u64 = 1024 * KB;
    const GB: u64 = 1024 * MB;
    const TB: u64 = 1024 * GB;
    if bytes >= TB {
        format!("{:.2} TB", bytes as f64 / TB as f64)
    } else if bytes >= GB {
        format!("{:.2} GB", bytes as f64 / GB as f64)
    } else if bytes >= MB {
        format!("{:.1} MB", bytes as f64 / MB as f64)
    } else if bytes >= KB {
        format!("{:.0} KB", bytes as f64 / KB as f64)
    } else {
        format!("{} B", bytes)
    }
}

fn query_volumes_for_drive(drive: &WmiDiskDrive) -> Vec<VolumeInfo> {
    let device_id = match drive.DeviceID.as_deref() {
        Some(id) => id,
        None => return vec![],
    };

    // Extract disk index from DeviceID like \\.\PHYSICALDRIVE2
    let disk_index = match device_id
        .to_uppercase()
        .rsplit("PHYSICALDRIVE")
        .next()
        .and_then(|s| s.parse::<u32>().ok())
    {
        Some(idx) => idx,
        None => {
            log_to_file(&format!(
                "ENRICH: can't extract disk index from {}",
                device_id
            ));
            return vec![];
        }
    };

    // Use PowerShell Get-Partition + Get-Volume (reliable on Win10/11)
    let ps_script = format!(
        "$ErrorActionPreference='SilentlyContinue'; \
         Get-Partition -DiskNumber {} | Where-Object {{ $_.DriveLetter }} | ForEach-Object {{ \
           $v = $_ | Get-Volume; \
           [PSCustomObject]@{{ \
             DriveLetter=[string]$_.DriveLetter; \
             Label=if($v.FileSystemLabel){{$v.FileSystemLabel}}else{{''}}; \
             Size=if($v.Size){{$v.Size}}else{{0}}; \
             FreeSpace=if($v.SizeRemaining){{$v.SizeRemaining}}else{{0}}; \
             FileSystem=if($v.FileSystem){{$v.FileSystem}}else{{''}} \
           }} \
         }} | ConvertTo-Json -Compress",
        disk_index
    );

    let output = match std::process::Command::new("powershell")
        .args(["-NoProfile", "-Command", &ps_script])
        .output()
    {
        Ok(o) => o,
        Err(e) => {
            log_to_file(&format!("ENRICH: PowerShell failed: {}", e));
            return vec![];
        }
    };

    let stdout = String::from_utf8_lossy(&output.stdout);
    let trimmed = stdout.trim();
    if trimmed.is_empty() {
        log_to_file(&format!(
            "ENRICH: no volumes for disk index {}",
            disk_index
        ));
        return vec![];
    }

    // PowerShell returns single object (not array) when only 1 result
    #[derive(Deserialize)]
    #[allow(non_snake_case)]
    struct PsVolume {
        DriveLetter: Option<String>,
        Label: Option<String>,
        Size: Option<u64>,
        FreeSpace: Option<u64>,
        FileSystem: Option<String>,
    }

    let ps_volumes: Vec<PsVolume> = match serde_json::from_str::<Vec<PsVolume>>(trimmed) {
        Ok(v) => v,
        Err(_) => match serde_json::from_str::<PsVolume>(trimmed) {
            Ok(v) => vec![v],
            Err(e) => {
                log_to_file(&format!(
                    "ENRICH: JSON parse failed: {} — raw: {}",
                    e, trimmed
                ));
                return vec![];
            }
        },
    };

    log_to_file(&format!(
        "ENRICH: disk index {} has {} volumes",
        disk_index,
        ps_volumes.len()
    ));

    ps_volumes
        .into_iter()
        .filter_map(|pv| {
            let letter = pv.DriveLetter?;
            if letter.is_empty() {
                return None;
            }
            Some(VolumeInfo {
                drive_letter: format!("{}:", letter),
                volume_name: pv.Label.unwrap_or_default(),
                total_bytes: pv.Size.unwrap_or(0),
                free_bytes: pv.FreeSpace.unwrap_or(0),
                file_system: pv.FileSystem.unwrap_or_default(),
                volume_serial: String::new(),
            })
        })
        .collect()
}

fn query_storage_info(wmi: &WMIConnection, device_id: &str) -> Option<StorageInfo> {
    // Extract USB serial suffix from device_id: USB\VID_xxxx&PID_yyyy\<serial>
    let usb_serial = device_id.rsplit('\\').next()?.to_uppercase();
    if usb_serial.is_empty() {
        return None;
    }

    // Query ALL disk drives — UAS drives show as InterfaceType=SCSI, not USB
    let drives: Vec<WmiDiskDrive> = match wmi.raw_query(
        "SELECT DeviceID, PNPDeviceID, Model, SerialNumber, Size, InterfaceType, \
         MediaType, Partitions, FirmwareRevision, Status \
         FROM Win32_DiskDrive",
    ) {
        Ok(d) => d,
        Err(e) => {
            log_to_file(&format!("ENRICH FAIL: WMI query error: {}", e));
            return None;
        }
    };

    log_to_file(&format!(
        "ENRICH: usb_serial={}, found {} drives: [{}]",
        usb_serial,
        drives.len(),
        drives
            .iter()
            .map(|d| format!(
                "{}|{}|{}",
                d.Model.as_deref().unwrap_or("?"),
                d.SerialNumber.as_deref().unwrap_or("?").trim(),
                d.InterfaceType.as_deref().unwrap_or("?")
            ))
            .collect::<Vec<_>>()
            .join(", ")
    ));

    if drives.is_empty() {
        return None;
    }

    // Match strategy (in priority order):
    // 1. DiskDrive serial is a substring of USB serial or vice versa
    //    e.g. USB serial "MSFT30NA8PE7LY" contains drive serial "NA8PE7LY"
    // 2. DiskDrive PNPDeviceID contains the USB serial suffix
    //    e.g. USBSTOR\...\WXF2D92FUV2S&0 contains "WXF2D92FUV2S"
    let matched = drives
        .iter()
        .find(|d| {
            // Check serial number match
            if let Some(serial) = &d.SerialNumber {
                let s = serial.trim().replace(' ', "").to_uppercase();
                if !s.is_empty() && (s.contains(&usb_serial) || usb_serial.contains(&s)) {
                    return true;
                }
            }
            // Check PNPDeviceID match (for USBSTOR drives)
            if let Some(pnp) = &d.PNPDeviceID {
                let p = pnp.to_uppercase();
                if p.contains(&usb_serial) {
                    return true;
                }
            }
            false
        });

    if matched.is_none() {
        log_to_file(&format!("ENRICH FAIL: no drive matched usb_serial={}", usb_serial));
        return None;
    }
    let matched = matched?;

    let volumes = query_volumes_for_drive(matched);
    log_to_file(&format!(
        "ENRICH: matched drive={} serial={} → {} volumes [{}]",
        matched.Model.as_deref().unwrap_or("?"),
        matched.SerialNumber.as_deref().unwrap_or("?").trim(),
        volumes.len(),
        volumes.iter().map(|v| v.drive_letter.as_str()).collect::<Vec<_>>().join(", ")
    ));

    Some(StorageInfo {
        model: matched.Model.clone().unwrap_or_default(),
        serial_number: matched
            .SerialNumber
            .clone()
            .unwrap_or_default()
            .trim()
            .to_string(),
        total_bytes: matched.Size.unwrap_or(0),
        interface_type: matched.InterfaceType.clone().unwrap_or_default(),
        media_type: matched.MediaType.clone().unwrap_or_default(),
        firmware: matched.FirmwareRevision.clone().unwrap_or_default(),
        partition_count: matched.Partitions.unwrap_or(0),
        status: matched.Status.clone().unwrap_or_default(),
        volumes,
    })
}

// ── Known device cache ──────────────────────────────────────────

#[derive(Serialize, Deserialize, Clone, Debug)]
struct KnownDevice {
    device_id: String,
    name: String,
    vid_pid: String,
    class: String,
    manufacturer: String,
    description: String,
    first_seen: String,
    last_seen: String,
    times_seen: u32,
    currently_connected: bool,
    #[serde(default)]
    nickname: Option<String>,
    #[serde(default)]
    storage_info: Option<StorageInfo>,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
struct KnownDeviceCache {
    version: u32,
    devices: HashMap<String, KnownDevice>,
}

impl KnownDeviceCache {
    fn new() -> Self {
        Self {
            version: 2,
            devices: HashMap::new(),
        }
    }
}

const CACHE_FILE: &str = "device-history-cache.json";

fn load_cache() -> KnownDeviceCache {
    std::fs::read_to_string(CACHE_FILE)
        .ok()
        .and_then(|s| serde_json::from_str(&s).ok())
        .unwrap_or_else(KnownDeviceCache::new)
}

fn save_cache(cache: &KnownDeviceCache) {
    if let Ok(json) = serde_json::to_string_pretty(cache) {
        let _ = std::fs::write(CACHE_FILE, json);
    }
}

// ── Shared state ───────────────────────────────────────────────

#[derive(Clone)]
struct DeviceEvent {
    timestamp: String,
    kind: EventKind,
    name: String,
    vid_pid: Option<String>,
    manufacturer: Option<String>,
    class: String,
    device_id: String,
}

#[derive(Clone, Copy, PartialEq)]
enum EventKind {
    Connect,
    Disconnect,
}

struct AppState {
    devices: Vec<(String, UsbDevice)>,
    events: Vec<DeviceEvent>,
    error: Option<String>,
    known_devices: KnownDeviceCache,
    storage_info: HashMap<String, StorageInfo>,
}

// ── Preferences ────────────────────────────────────────────────

const PREFS_FILE: &str = "device-history.prefs";

struct Prefs {
    about_open: bool,
    theme: String,
    active_tab: String,
}

impl Prefs {
    fn load() -> Self {
        let defaults = Self {
            about_open: true,
            theme: "Neon".to_string(),
            active_tab: "Monitor".to_string(),
        };
        let Ok(content) = std::fs::read_to_string(PREFS_FILE) else {
            return defaults;
        };
        let mut prefs = defaults;
        for line in content.lines() {
            if let Some((key, val)) = line.split_once('=') {
                match key.trim() {
                    "about_open" => prefs.about_open = val.trim() == "true",
                    "theme" => prefs.theme = val.trim().to_string(),
                    "active_tab" => prefs.active_tab = val.trim().to_string(),
                    _ => {}
                }
            }
        }
        prefs
    }

    fn save(&self) {
        let content = format!(
            "about_open={}\ntheme={}\nactive_tab={}\n",
            self.about_open, self.theme, self.active_tab
        );
        let _ = std::fs::write(PREFS_FILE, content);
    }
}

fn log_to_file(msg: &str) {
    let path = "device-history.log";
    if let Ok(mut f) = OpenOptions::new().create(true).append(true).open(path) {
        let ts = Local::now().format("%Y-%m-%d %H:%M:%S%.3f");
        let _ = writeln!(f, "[{}] {}", ts, msg);
    }
}

// ── Background monitor thread ──────────────────────────────────

fn monitor_loop(state: Arc<Mutex<AppState>>) {
    let com = match COMLibrary::new() {
        Ok(c) => c,
        Err(e) => {
            if let Ok(mut s) = state.lock() {
                s.error = Some(format!("COM init failed: {}", e));
            }
            return;
        }
    };
    let wmi = match WMIConnection::new(com) {
        Ok(w) => w,
        Err(e) => {
            if let Ok(mut s) = state.lock() {
                s.error = Some(format!("WMI connect failed: {}", e));
            }
            return;
        }
    };

    let mut prev = match query_devices(&wmi) {
        Some(d) => d,
        None => {
            if let Ok(mut s) = state.lock() {
                s.error = Some("Failed to query USB devices".into());
            }
            return;
        }
    };

    // Initial snapshot — merge into cache
    {
        let now = Local::now().format("%Y-%m-%d %H:%M:%S").to_string();
        if let Ok(mut s) = state.lock() {
            // Mark all cached devices as offline
            for dev in s.known_devices.devices.values_mut() {
                dev.currently_connected = false;
            }

            // Merge current devices into cache
            for (id, dev) in &prev {
                let is_new = !s.known_devices.devices.contains_key(id);
                let entry =
                    s.known_devices
                        .devices
                        .entry(id.clone())
                        .or_insert_with(|| KnownDevice {
                            device_id: id.clone(),
                            name: dev.display_name().to_string(),
                            vid_pid: dev.vid_pid().unwrap_or_default(),
                            class: dev.class().to_string(),
                            manufacturer: dev.Manufacturer.clone().unwrap_or_default(),
                            description: dev.Description.clone().unwrap_or_default(),
                            first_seen: now.clone(),
                            last_seen: now.clone(),
                            times_seen: 1,
                            currently_connected: true,
                            nickname: None,
                            storage_info: None,
                        });
                if !is_new {
                    entry.last_seen = now.clone();
                    entry.currently_connected = true;
                    entry.name = dev.display_name().to_string();
                    entry.vid_pid = dev.vid_pid().unwrap_or_default();
                    entry.class = dev.class().to_string();
                    entry.manufacturer = dev.Manufacturer.clone().unwrap_or_default();
                    entry.description = dev.Description.clone().unwrap_or_default();
                }
            }

            let mut sorted: Vec<_> =
                prev.iter().map(|(id, d)| (id.clone(), d.clone())).collect();
            sorted.sort_by(|a, b| {
                a.1.display_name()
                    .to_lowercase()
                    .cmp(&b.1.display_name().to_lowercase())
            });
            s.devices = sorted;

            save_cache(&s.known_devices);
        }
    }

    // Initial enrichment for connected storage devices
    for (id, dev) in &prev {
        if is_storage_device(dev) {
            if let Some(info) = query_storage_info(&wmi, id) {
                log_to_file(&format!(
                    "ENRICHED (startup): {} → {} [{}]",
                    id,
                    info.model,
                    info.volumes
                        .iter()
                        .map(|v| v.drive_letter.as_str())
                        .collect::<Vec<_>>()
                        .join(", ")
                ));
                if let Ok(mut s) = state.lock() {
                    s.storage_info.insert(id.clone(), info.clone());
                    if let Some(kd) = s.known_devices.devices.get_mut(id) {
                        kd.storage_info = Some(info);
                    }
                    save_cache(&s.known_devices);
                }
            }
        }
    }

    log_to_file(&format!("Started monitoring — {} devices", prev.len()));

    let mut pending_enrichments: Vec<(String, Instant)> = Vec::new();

    loop {
        thread::sleep(Duration::from_millis(500));

        // Process pending enrichments (2s delay for drives to mount)
        let now_instant = Instant::now();
        let ready: Vec<String> = pending_enrichments
            .iter()
            .filter(|(_, scheduled)| now_instant.duration_since(*scheduled) >= Duration::from_secs(2))
            .map(|(id, _)| id.clone())
            .collect();
        pending_enrichments
            .retain(|(_, scheduled)| now_instant.duration_since(*scheduled) < Duration::from_secs(2));
        for enrich_id in ready {
            if let Some(info) = query_storage_info(&wmi, &enrich_id) {
                log_to_file(&format!(
                    "ENRICHED: {} → {} [{}]",
                    enrich_id,
                    info.model,
                    info.volumes
                        .iter()
                        .map(|v| v.drive_letter.as_str())
                        .collect::<Vec<_>>()
                        .join(", ")
                ));
                if let Ok(mut s) = state.lock() {
                    s.storage_info.insert(enrich_id.clone(), info.clone());
                    if let Some(kd) = s.known_devices.devices.get_mut(&enrich_id) {
                        kd.storage_info = Some(info);
                    }
                    save_cache(&s.known_devices);
                }
            }
        }

        let Some(current) = query_devices(&wmi) else {
            continue;
        };

        let mut new_events = Vec::new();
        let ts = Local::now().format("%H:%M:%S").to_string();
        let now_iso = Local::now().format("%Y-%m-%d %H:%M:%S").to_string();

        for (id, dev) in &prev {
            if !current.contains_key(id) {
                let event = DeviceEvent {
                    timestamp: ts.clone(),
                    kind: EventKind::Disconnect,
                    name: dev.display_name().to_string(),
                    vid_pid: dev.vid_pid(),
                    manufacturer: dev.Manufacturer.clone(),
                    class: dev.class().to_string(),
                    device_id: id.clone(),
                };
                log_to_file(&format!(
                    "DISCONNECT: {} [{}] | {}",
                    event.name,
                    event.vid_pid.as_deref().unwrap_or("?"),
                    id
                ));
                new_events.push(event);
            }
        }

        for (id, dev) in &current {
            if !prev.contains_key(id) {
                let event = DeviceEvent {
                    timestamp: ts.clone(),
                    kind: EventKind::Connect,
                    name: dev.display_name().to_string(),
                    vid_pid: dev.vid_pid(),
                    manufacturer: dev.Manufacturer.clone(),
                    class: dev.class().to_string(),
                    device_id: id.clone(),
                };
                log_to_file(&format!(
                    "CONNECT: {} [{}] | {}",
                    event.name,
                    event.vid_pid.as_deref().unwrap_or("?"),
                    id
                ));
                new_events.push(event);
            }
        }

        if !new_events.is_empty() {
            // Collect IDs for enrichment/cleanup before consuming events
            let enrich_ids: Vec<String> = new_events
                .iter()
                .filter(|e| e.kind == EventKind::Connect)
                .filter(|e| {
                    current
                        .get(&e.device_id)
                        .map_or(false, |d| is_storage_device(d))
                })
                .map(|e| e.device_id.clone())
                .collect();
            if let Ok(mut s) = state.lock() {
                // Update cache for each event
                for event in &new_events {
                    match event.kind {
                        EventKind::Connect => {
                            if let Some(dev) = current.get(&event.device_id) {
                                let is_new =
                                    !s.known_devices.devices.contains_key(&event.device_id);
                                let entry = s
                                    .known_devices
                                    .devices
                                    .entry(event.device_id.clone())
                                    .or_insert_with(|| KnownDevice {
                                        device_id: event.device_id.clone(),
                                        name: dev.display_name().to_string(),
                                        vid_pid: dev.vid_pid().unwrap_or_default(),
                                        class: dev.class().to_string(),
                                        manufacturer: dev.Manufacturer.clone().unwrap_or_default(),
                                        description: dev.Description.clone().unwrap_or_default(),
                                        first_seen: now_iso.clone(),
                                        last_seen: now_iso.clone(),
                                        times_seen: 0,
                                        currently_connected: true,
                                        nickname: None,
                                        storage_info: None,
                                    });
                                entry.times_seen += 1;
                                entry.last_seen = now_iso.clone();
                                entry.currently_connected = true;
                                if !is_new {
                                    entry.name = dev.display_name().to_string();
                                    entry.vid_pid = dev.vid_pid().unwrap_or_default();
                                    entry.class = dev.class().to_string();
                                    entry.manufacturer =
                                        dev.Manufacturer.clone().unwrap_or_default();
                                    entry.description =
                                        dev.Description.clone().unwrap_or_default();
                                }
                            }
                        }
                        EventKind::Disconnect => {
                            if let Some(entry) =
                                s.known_devices.devices.get_mut(&event.device_id)
                            {
                                entry.last_seen = now_iso.clone();
                                entry.currently_connected = false;
                            }
                            // Remove live storage info on disconnect
                            s.storage_info.remove(&event.device_id);
                        }
                    }
                }

                s.events.extend(new_events);
                let mut sorted: Vec<_> =
                    current.iter().map(|(id, d)| (id.clone(), d.clone())).collect();
                sorted.sort_by(|a, b| {
                    a.1.display_name()
                        .to_lowercase()
                        .cmp(&b.1.display_name().to_lowercase())
                });
                s.devices = sorted;

                save_cache(&s.known_devices);
            }

            // Schedule enrichment for connected storage devices (2s delay)
            for id in enrich_ids {
                pending_enrichments.push((id, Instant::now()));
            }
        }

        prev = current;
    }
}

// ── Theme system ───────────────────────────────────────────────

#[derive(Clone, Copy, PartialEq)]
enum Theme {
    Neon,
    Light,
    Mids,
}

impl Theme {
    fn label(self) -> &'static str {
        match self {
            Theme::Neon => "Neon",
            Theme::Light => "Light",
            Theme::Mids => "Mids",
        }
    }

    fn from_label(s: &str) -> Self {
        match s {
            "Light" => Theme::Light,
            "Mids" => Theme::Mids,
            _ => Theme::Neon,
        }
    }

    fn colors(self) -> ThemeColors {
        match self {
            Theme::Neon => ThemeColors {
                bg_deep: c(0x0d, 0x0f, 0x14),
                bg_surface: c(0x1a, 0x1c, 0x23),
                bg_elevated: c(0x22, 0x25, 0x2e),
                border: c(0x2a, 0x2d, 0x3a),
                accent: c(0xa8, 0x55, 0xf7),
                orange: c(0xff, 0x8b, 0x3d),
                teal: c(0x2e, 0xe6, 0xd7),
                green: c(0x50, 0xfa, 0x7b),
                red: c(0xff, 0x55, 0x55),
                yellow: c(0xf1, 0xfa, 0x8c),
                pink: c(0xff, 0x79, 0xc6),
                cyan: c(0x8b, 0xe9, 0xfd),
                text: c(0xe8, 0xe8, 0xf0),
                text_sec: c(0x8c, 0x8c, 0xa0),
                text_muted: c(0x72, 0x74, 0x88),
                dark_mode: true,
            },
            Theme::Light => ThemeColors {
                bg_deep: c(0xf2, 0xf2, 0xf7),
                bg_surface: c(0xff, 0xff, 0xff),
                bg_elevated: c(0xe8, 0xe8, 0xf0),
                border: c(0xd0, 0xd0, 0xdd),
                accent: c(0x7c, 0x3a, 0xed),
                orange: c(0xea, 0x58, 0x0c),
                teal: c(0x0d, 0x94, 0x88),
                green: c(0x16, 0xa3, 0x4a),
                red: c(0xdc, 0x26, 0x26),
                yellow: c(0xa1, 0x6b, 0x07),
                pink: c(0xdb, 0x27, 0x77),
                cyan: c(0x06, 0x7a, 0x99),
                text: c(0x1a, 0x1a, 0x2e),
                text_sec: c(0x64, 0x74, 0x8b),
                text_muted: c(0x94, 0xa3, 0xb8),
                dark_mode: false,
            },
            Theme::Mids => ThemeColors {
                bg_deep: c(0x2a, 0x2c, 0x35),
                bg_surface: c(0x36, 0x38, 0x44),
                bg_elevated: c(0x42, 0x44, 0x52),
                border: c(0x52, 0x54, 0x66),
                accent: c(0x9b, 0x7d, 0xff),
                orange: c(0xff, 0x9f, 0x5a),
                teal: c(0x4d, 0xd8, 0xcc),
                green: c(0x6b, 0xe8, 0x8a),
                red: c(0xff, 0x6e, 0x6e),
                yellow: c(0xe8, 0xd4, 0x6e),
                pink: c(0xff, 0x8f, 0xd0),
                cyan: c(0x7d, 0xcc, 0xe8),
                text: c(0xd8, 0xd8, 0xe4),
                text_sec: c(0x98, 0x98, 0xac),
                text_muted: c(0x6e, 0x70, 0x82),
                dark_mode: true,
            },
        }
    }
}

const fn c(r: u8, g: u8, b: u8) -> egui::Color32 {
    egui::Color32::from_rgb(r, g, b)
}

#[derive(Clone, Copy)]
struct ThemeColors {
    bg_deep: egui::Color32,
    bg_surface: egui::Color32,
    bg_elevated: egui::Color32,
    border: egui::Color32,
    accent: egui::Color32,
    orange: egui::Color32,
    teal: egui::Color32,
    green: egui::Color32,
    red: egui::Color32,
    yellow: egui::Color32,
    pink: egui::Color32,
    cyan: egui::Color32,
    text: egui::Color32,
    text_sec: egui::Color32,
    text_muted: egui::Color32,
    dark_mode: bool,
}

// ── Helpers ────────────────────────────────────────────────────

fn blend(base: egui::Color32, target: egui::Color32, t: f32) -> egui::Color32 {
    let m = |a: u8, b: u8| (a as f32 * (1.0 - t) + b as f32 * t).clamp(0.0, 255.0) as u8;
    egui::Color32::from_rgb(
        m(base.r(), target.r()),
        m(base.g(), target.g()),
        m(base.b(), target.b()),
    )
}

fn load_icon() -> Option<egui::IconData> {
    let png_bytes = include_bytes!("../assets/icon.png");
    let img = image::load_from_memory(png_bytes).ok()?.into_rgba8();
    let (w, h) = img.dimensions();
    Some(egui::IconData {
        rgba: img.into_raw(),
        width: w,
        height: h,
    })
}

fn apply_theme(ctx: &egui::Context, tc: &ThemeColors) {
    ctx.set_visuals({
        let mut v = if tc.dark_mode {
            egui::Visuals::dark()
        } else {
            egui::Visuals::light()
        };

        v.panel_fill = tc.bg_deep;
        v.window_fill = tc.bg_surface;
        v.extreme_bg_color = tc.bg_deep;
        v.faint_bg_color = tc.bg_elevated;

        v.widgets.noninteractive.bg_fill = tc.bg_surface;
        v.widgets.noninteractive.fg_stroke = egui::Stroke::new(1.0, tc.text_sec);
        v.widgets.noninteractive.bg_stroke = egui::Stroke::new(0.5, tc.border);
        v.widgets.noninteractive.rounding = egui::Rounding::same(4.0);

        v.widgets.inactive.bg_fill = tc.bg_elevated;
        v.widgets.inactive.fg_stroke = egui::Stroke::new(1.0, tc.text);
        v.widgets.inactive.bg_stroke = egui::Stroke::new(0.5, tc.border);
        v.widgets.inactive.rounding = egui::Rounding::same(4.0);

        v.widgets.hovered.bg_fill = blend(tc.bg_elevated, tc.accent, 0.15);
        v.widgets.hovered.fg_stroke = egui::Stroke::new(1.0, tc.text);
        v.widgets.hovered.bg_stroke = egui::Stroke::new(1.0, tc.accent);
        v.widgets.hovered.rounding = egui::Rounding::same(4.0);

        v.widgets.active.bg_fill = blend(tc.bg_elevated, tc.accent, 0.25);
        v.widgets.active.fg_stroke = egui::Stroke::new(1.0, tc.text);
        v.widgets.active.bg_stroke = egui::Stroke::new(1.5, tc.accent);
        v.widgets.active.rounding = egui::Rounding::same(4.0);

        v.selection.bg_fill = blend(tc.bg_surface, tc.accent, 0.2);
        v.selection.stroke = egui::Stroke::new(1.0, tc.accent);

        v.window_rounding = egui::Rounding::same(6.0);
        v.window_shadow = egui::Shadow {
            offset: egui::Vec2::new(0.0, 2.0),
            blur: 8.0,
            spread: 0.0,
            color: egui::Color32::from_black_alpha(if tc.dark_mode { 80 } else { 30 }),
        };

        v.collapsing_header_frame = true;
        v.override_text_color = Some(tc.text);

        v
    });
}

fn draw_rainbow_separator(ui: &mut egui::Ui, tc: &ThemeColors) {
    let colors = [
        tc.red, tc.orange, tc.yellow, tc.green, tc.teal, tc.cyan, tc.accent, tc.pink,
    ];
    let w = ui.available_width();
    let h = 2.0;
    let (rect, _) = ui.allocate_exact_size(egui::Vec2::new(w, h), egui::Sense::hover());
    let painter = ui.painter_at(rect);
    let seg_w = w / colors.len() as f32;

    for i in 0..colors.len() {
        let c0 = colors[i];
        let c1 = colors[(i + 1) % colors.len()];
        let subs = 8;
        let sub_w = seg_w / subs as f32;
        for s in 0..subs {
            let t = s as f32 / subs as f32;
            let color = blend(c0, c1, t);
            let x = rect.left() + i as f32 * seg_w + s as f32 * sub_w;
            painter.rect_filled(
                egui::Rect::from_min_size(
                    egui::Pos2::new(x, rect.top()),
                    egui::Vec2::new(sub_w + 0.5, h),
                ),
                0.0,
                color,
            );
        }
    }
}

// ── Device Detail Panel ─────────────────────────────────────────

#[allow(clippy::too_many_arguments)]
fn draw_device_detail_panel(
    ui: &mut egui::Ui,
    tc: &ThemeColors,
    device_id: &str,
    device_name: &str,
    vid_pid: Option<String>,
    class: &str,
    manufacturer: Option<&str>,
    description: Option<&str>,
    known_device: Option<&KnownDevice>,
    storage_info: Option<&StorageInfo>,
    nickname_buf: &mut String,
    state_arc: &Arc<Mutex<AppState>>,
    is_connected: bool,
) {
    let detail_frame = egui::Frame::none()
        .fill(blend(tc.bg_surface, tc.accent, 0.03))
        .rounding(egui::Rounding {
            nw: 0.0,
            ne: 0.0,
            sw: 6.0,
            se: 6.0,
        })
        .stroke(egui::Stroke::new(1.0, blend(tc.border, tc.accent, 0.3)))
        .inner_margin(egui::Margin::same(12.0));

    detail_frame.show(ui, |ui: &mut egui::Ui| {
        ui.set_width(ui.available_width());
        ui.spacing_mut().item_spacing.y = 4.0;

        // ── Header: drive info FIRST if storage device ──
        if let Some(si) = storage_info {
            // Big drive letter + volume name as the headline
            for vol in &si.volumes {
                ui.horizontal(|ui| {
                    ui.label(
                        egui::RichText::new(&vol.drive_letter)
                            .color(tc.green)
                            .strong()
                            .monospace()
                            .size(20.0),
                    );
                    if !vol.volume_name.is_empty() {
                        ui.label(
                            egui::RichText::new(format!("\"{}\"", vol.volume_name))
                                .color(tc.text)
                                .strong()
                                .size(16.0),
                        );
                    }
                    ui.label(
                        egui::RichText::new(format!("({})", vol.file_system))
                            .color(tc.text_sec)
                            .size(12.0),
                    );
                });

                // Capacity bar right under the headline
                if vol.total_bytes > 0 {
                    let free_str = format_bytes(vol.free_bytes);
                    let total_str = format_bytes(vol.total_bytes);
                    let used_frac = 1.0 - (vol.free_bytes as f32 / vol.total_bytes as f32);
                    let bar_color = if used_frac < 0.7 {
                        tc.green
                    } else if used_frac < 0.9 {
                        tc.yellow
                    } else {
                        tc.red
                    };
                    let bar_width = (ui.available_width() - 10.0).max(100.0);
                    let bar_height = 10.0;
                    let (bar_rect, _) = ui.allocate_exact_size(
                        egui::Vec2::new(bar_width, bar_height),
                        egui::Sense::hover(),
                    );
                    let painter = ui.painter_at(bar_rect);
                    painter.rect_filled(bar_rect, 4.0, tc.bg_elevated);
                    let filled_rect = egui::Rect::from_min_size(
                        bar_rect.left_top(),
                        egui::Vec2::new(bar_width * used_frac, bar_height),
                    );
                    painter.rect_filled(filled_rect, 4.0, bar_color);
                    ui.label(
                        egui::RichText::new(format!(
                            "{} free / {}  ({:.0}% used)",
                            free_str, total_str, used_frac * 100.0
                        ))
                        .color(tc.text_sec)
                        .size(11.0),
                    );
                }
            }

            // Model + serial on one line
            ui.horizontal(|ui| {
                ui.label(
                    egui::RichText::new(&si.model)
                        .color(tc.text)
                        .size(12.0),
                );
                if !si.serial_number.is_empty() {
                    ui.label(
                        egui::RichText::new(format!("({})", si.serial_number))
                            .color(tc.text_sec)
                            .monospace()
                            .size(11.0),
                    );
                }
            });

            if !is_connected {
                ui.label(
                    egui::RichText::new("OFFLINE -- showing last known info")
                        .color(tc.orange)
                        .italics()
                        .size(10.0),
                );
            }

            ui.add_space(2.0);
            let sep_rect = ui.allocate_exact_size(
                egui::Vec2::new(ui.available_width(), 1.0),
                egui::Sense::hover(),
            ).0;
            ui.painter().rect_filled(sep_rect, 0.0, tc.border);
            ui.add_space(2.0);
        } else {
            // Non-storage device: just show name as header
            ui.label(
                egui::RichText::new(device_name)
                    .strong()
                    .size(14.0)
                    .color(tc.text),
            );
        }

        // ── Nickname editing ──
        ui.horizontal(|ui| {
            ui.label(
                egui::RichText::new("Nickname:")
                    .color(tc.text_sec)
                    .size(11.0),
            );
            let te = egui::TextEdit::singleline(nickname_buf)
                .hint_text("e.g. My 4TB Seagate")
                .desired_width(200.0)
                .text_color(tc.text);
            ui.add(te);
            let save_btn = egui::Button::new(
                egui::RichText::new("Save").color(tc.teal).size(11.0),
            )
            .fill(egui::Color32::TRANSPARENT)
            .stroke(egui::Stroke::new(0.5, tc.teal))
            .rounding(3.0);
            if ui.add(save_btn).clicked() {
                let nick = if nickname_buf.trim().is_empty() {
                    None
                } else {
                    Some(nickname_buf.trim().to_string())
                };
                if let Ok(mut s) = state_arc.lock() {
                    if let Some(kd) = s.known_devices.devices.get_mut(device_id) {
                        kd.nickname = nick;
                    }
                    save_cache(&s.known_devices);
                }
            }
        });

        ui.add_space(4.0);

        // ── STORAGE technical details ──
        if let Some(si) = storage_info {
            // Compact detail row: interface, firmware, partitions, status
            let detail_rows = [
                ("Interface:", si.interface_type.as_str()),
                ("Firmware:", si.firmware.as_str()),
                ("Status:", si.status.as_str()),
            ];
            for (label, value) in &detail_rows {
                if !value.is_empty() {
                    ui.horizontal(|ui| {
                        ui.label(
                            egui::RichText::new(*label)
                                .color(tc.text_sec)
                                .size(11.0),
                        );
                        ui.label(
                            egui::RichText::new(*value)
                                .color(tc.text)
                                .monospace()
                                .size(11.0),
                        );
                    });
                }
            }

            ui.add_space(4.0);
            // Thin separator
            let sep_rect = ui.allocate_exact_size(
                egui::Vec2::new(ui.available_width(), 1.0),
                egui::Sense::hover(),
            ).0;
            ui.painter().rect_filled(sep_rect, 0.0, tc.border);
            ui.add_space(4.0);
        }

        // ── DEVICE INFO section ──
        ui.label(
            egui::RichText::new("DEVICE INFO")
                .strong()
                .size(12.0)
                .color(tc.cyan),
        );

        let info_rows: Vec<(&str, String)> = vec![
            ("Device ID:", device_id.to_string()),
            (
                "VID:PID:",
                vid_pid.clone().unwrap_or_else(|| "-".to_string()),
            ),
            ("Class:", class.to_string()),
            (
                "Manufacturer:",
                manufacturer.unwrap_or("-").to_string(),
            ),
            (
                "Description:",
                description.unwrap_or("-").to_string(),
            ),
        ];
        for (label, value) in &info_rows {
            ui.horizontal(|ui| {
                ui.label(
                    egui::RichText::new(*label)
                        .color(tc.text_sec)
                        .size(11.0),
                );
                ui.add(
                    egui::Label::new(
                        egui::RichText::new(value)
                            .color(tc.text)
                            .monospace()
                            .size(11.0),
                    )
                    .truncate(),
                );
            });
        }

        // ── HISTORY section ──
        if let Some(kd) = known_device {
            ui.add_space(4.0);
            let sep_rect = ui.allocate_exact_size(
                egui::Vec2::new(ui.available_width(), 1.0),
                egui::Sense::hover(),
            ).0;
            ui.painter().rect_filled(sep_rect, 0.0, tc.border);
            ui.add_space(4.0);

            ui.label(
                egui::RichText::new("HISTORY")
                    .strong()
                    .size(12.0)
                    .color(tc.cyan),
            );
            ui.horizontal(|ui| {
                ui.spacing_mut().item_spacing.x = 16.0;
                ui.label(
                    egui::RichText::new(format!("First seen: {}", kd.first_seen))
                        .color(tc.text_sec)
                        .size(11.0),
                );
                ui.label(
                    egui::RichText::new(format!("Last seen: {}", kd.last_seen))
                        .color(tc.text_sec)
                        .size(11.0),
                );
                ui.label(
                    egui::RichText::new(format!("Times seen: {}x", kd.times_seen))
                        .color(tc.teal)
                        .size(11.0),
                );
            });
        }

        // ── Action buttons ──
        ui.add_space(6.0);
        let sep_rect = ui.allocate_exact_size(
            egui::Vec2::new(ui.available_width(), 1.0),
            egui::Sense::hover(),
        ).0;
        ui.painter().rect_filled(sep_rect, 0.0, tc.border);
        ui.add_space(4.0);

        ui.horizontal(|ui| {
            ui.spacing_mut().item_spacing.x = 8.0;

            // Copy Device ID
            let copy_id_btn = egui::Button::new(
                egui::RichText::new("Copy Device ID")
                    .color(tc.text_sec)
                    .size(11.0),
            )
            .fill(egui::Color32::TRANSPARENT)
            .stroke(egui::Stroke::new(0.5, tc.border))
            .rounding(3.0);
            if ui.add(copy_id_btn).clicked() {
                ui.output_mut(|o| {
                    o.copied_text = device_id.to_string();
                });
            }

            // Copy Serial (if storage info available)
            if let Some(si) = storage_info {
                if !si.serial_number.is_empty() {
                    let copy_serial_btn = egui::Button::new(
                        egui::RichText::new("Copy Serial")
                            .color(tc.text_sec)
                            .size(11.0),
                    )
                    .fill(egui::Color32::TRANSPARENT)
                    .stroke(egui::Stroke::new(0.5, tc.border))
                    .rounding(3.0);
                    if ui.add(copy_serial_btn).clicked() {
                        ui.output_mut(|o| {
                            o.copied_text = si.serial_number.clone();
                        });
                    }
                }
            }

            // Forget button
            let forget_btn = egui::Button::new(
                egui::RichText::new("Forget")
                    .color(tc.red)
                    .size(11.0),
            )
            .fill(egui::Color32::TRANSPARENT)
            .stroke(egui::Stroke::new(0.5, tc.red))
            .rounding(3.0);
            if ui.add(forget_btn).clicked() {
                if let Ok(mut s) = state_arc.lock() {
                    s.known_devices.devices.remove(device_id);
                    s.storage_info.remove(device_id);
                    save_cache(&s.known_devices);
                }
            }
        });
    });
}

// ── Tab + Sort enums ───────────────────────────────────────────

#[derive(Clone, Copy, PartialEq)]
enum ActiveTab {
    Monitor,
    KnownDevices,
}

impl ActiveTab {
    fn label(self) -> &'static str {
        match self {
            ActiveTab::Monitor => "Monitor",
            ActiveTab::KnownDevices => "Known Devices",
        }
    }

    fn from_label(s: &str) -> Self {
        match s {
            "KnownDevices" => ActiveTab::KnownDevices,
            _ => ActiveTab::Monitor,
        }
    }

    fn save_key(self) -> &'static str {
        match self {
            ActiveTab::Monitor => "Monitor",
            ActiveTab::KnownDevices => "KnownDevices",
        }
    }
}

#[derive(Clone, Copy, PartialEq)]
enum SortMode {
    Status,
    Name,
    LastSeen,
    TimesSeen,
    FirstSeen,
}

impl SortMode {
    fn label(self) -> &'static str {
        match self {
            SortMode::Status => "Status",
            SortMode::Name => "Name",
            SortMode::LastSeen => "Last Seen",
            SortMode::TimesSeen => "Times Seen",
            SortMode::FirstSeen => "First Seen",
        }
    }
}

// ── GUI ────────────────────────────────────────────────────────

struct TrayMenuIds {
    show: tray_icon::menu::MenuId,
    hide: tray_icon::menu::MenuId,
    exit: tray_icon::menu::MenuId,
}

struct DeviceHistoryApp {
    state: Arc<Mutex<AppState>>,
    theme: Theme,
    colors: ThemeColors,
    needs_theme_apply: bool,
    show_about: bool,
    update_available: Arc<Mutex<Option<String>>>,
    tray_menu_ids: TrayMenuIds,
    hidden: bool,
    active_tab: ActiveTab,
    search_query: String,
    sort_mode: SortMode,
    sort_ascending: bool,
    selected_device: Option<String>,
    nickname_buf: String,
}

impl DeviceHistoryApp {
    fn new(state: Arc<Mutex<AppState>>, tray_menu_ids: TrayMenuIds) -> Self {
        let prefs = Prefs::load();
        let theme = Theme::from_label(&prefs.theme);
        let update_available = Arc::new(Mutex::new(None));

        // Background update check
        let update_flag = update_available.clone();
        thread::spawn(move || {
            let current = env!("CARGO_PKG_VERSION");
            let resp = ureq::get(
                "https://api.github.com/repos/TrentSterling/device-history/releases/latest",
            )
            .set("User-Agent", "device-history")
            .call();
            if let Ok(resp) = resp {
                if let Ok(body) = resp.into_string() {
                    if let Some(start) = body.find("\"tag_name\"") {
                        let rest = &body[start..];
                        if let Some(colon) = rest.find(':') {
                            let after_colon = rest[colon + 1..].trim_start();
                            if after_colon.starts_with('"') {
                                let val_start = 1;
                                if let Some(val_end) = after_colon[val_start..].find('"') {
                                    let tag = &after_colon[val_start..val_start + val_end];
                                    let latest = tag.trim_start_matches('v');
                                    if latest != current {
                                        if let Ok(mut u) = update_flag.lock() {
                                            *u = Some(latest.to_string());
                                        }
                                    }
                                }
                            }
                        }
                    }
                }
            }
        });

        Self {
            state,
            theme,
            colors: theme.colors(),
            needs_theme_apply: true,
            show_about: prefs.about_open,
            update_available,
            tray_menu_ids,
            hidden: false,
            active_tab: ActiveTab::from_label(&prefs.active_tab),
            search_query: String::new(),
            sort_mode: SortMode::Status,
            sort_ascending: true,
            selected_device: None,
            nickname_buf: String::new(),
        }
    }

    fn save_prefs(&self) {
        let prefs = Prefs {
            about_open: self.show_about,
            theme: self.theme.label().to_string(),
            active_tab: self.active_tab.save_key().to_string(),
        };
        prefs.save();
    }
}

impl eframe::App for DeviceHistoryApp {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        // ── Intercept close button → hide to tray ──
        if ctx.input(|i| i.viewport().close_requested()) {
            ctx.send_viewport_cmd(egui::ViewportCommand::CancelClose);
            #[cfg(windows)]
            win32::hide_window();
            self.hidden = true;
        }

        if self.needs_theme_apply {
            apply_theme(ctx, &self.colors);
            self.needs_theme_apply = false;
        }

        ctx.request_repaint_after(Duration::from_millis(250));

        let tc = self.colors;
        let state_arc = self.state.clone();

        // ── Clone all data from state, drop lock ──
        let (events, devices, known_devices, error, storage_info) = {
            let s = self.state.lock().unwrap();
            (
                s.events.clone(),
                s.devices.clone(),
                s.known_devices.clone(),
                s.error.clone(),
                s.storage_info.clone(),
            )
        };

        let known_total = known_devices.devices.len();
        let known_online = known_devices
            .devices
            .values()
            .filter(|d| d.currently_connected)
            .count();

        // ── Header ──
        let mut new_theme: Option<Theme> = None;
        egui::TopBottomPanel::top("header")
            .frame(
                egui::Frame::none()
                    .fill(tc.bg_surface)
                    .inner_margin(egui::Margin::symmetric(14.0, 10.0))
                    .stroke(egui::Stroke::new(1.0, tc.border)),
            )
            .show(ctx, |ui| {
                ui.horizontal(|ui| {
                    ui.label(
                        egui::RichText::new("Device History")
                            .strong()
                            .size(20.0)
                            .color(tc.accent),
                    );
                    ui.add_space(4.0);
                    ui.label(
                        egui::RichText::new(format!("v{}", env!("CARGO_PKG_VERSION")))
                            .size(11.0)
                            .color(tc.text_muted),
                    );

                    // Update available banner
                    if let Ok(guard) = self.update_available.lock() {
                        if let Some(ver) = guard.as_ref() {
                            ui.add_space(8.0);
                            let btn = egui::Button::new(
                                egui::RichText::new(format!("Update: v{}", ver))
                                    .size(11.0)
                                    .color(tc.orange),
                            )
                            .fill(egui::Color32::TRANSPARENT)
                            .stroke(egui::Stroke::new(1.0, tc.orange))
                            .rounding(4.0);
                            if ui.add(btn).clicked() {
                                let _ = open::that(
                                    "https://github.com/TrentSterling/device-history/releases/latest",
                                );
                            }
                        }
                    }

                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        // Status (rightmost)
                        if let Some(err) = &error {
                            ui.label(egui::RichText::new(err).color(tc.pink).size(13.0));
                        } else {
                            ui.label(
                                egui::RichText::new(format!(
                                    "Monitoring {} devices",
                                    devices.len()
                                ))
                                .color(tc.teal)
                                .size(13.0),
                            );
                        }

                        ui.add_space(12.0);
                        ui.separator();
                        ui.add_space(4.0);

                        // Theme picker
                        for t in [Theme::Neon, Theme::Light, Theme::Mids] {
                            let selected = self.theme == t;
                            let label_color = if selected { tc.accent } else { tc.text_sec };
                            let btn = egui::Button::new(
                                egui::RichText::new(t.label()).size(11.0).color(label_color),
                            )
                            .fill(if selected {
                                blend(tc.bg_elevated, tc.accent, 0.12)
                            } else {
                                egui::Color32::TRANSPARENT
                            })
                            .stroke(if selected {
                                egui::Stroke::new(1.0, tc.accent)
                            } else {
                                egui::Stroke::new(0.5, tc.border)
                            })
                            .rounding(3.0);

                            if ui.add(btn).clicked() && !selected {
                                new_theme = Some(t);
                            }
                        }
                    });
                });
            });

        // Apply theme change
        if let Some(t) = new_theme {
            self.theme = t;
            self.colors = t.colors();
            self.needs_theme_apply = true;
            self.save_prefs();
        }

        // ── Tab bar ──
        let mut new_tab = self.active_tab;
        egui::TopBottomPanel::top("tabs")
            .frame(
                egui::Frame::none()
                    .fill(tc.bg_surface)
                    .inner_margin(egui::Margin {
                        left: 14.0,
                        right: 14.0,
                        top: 6.0,
                        bottom: 0.0,
                    })
                    .stroke(egui::Stroke::new(0.5, tc.border)),
            )
            .show(ctx, |ui| {
                ui.horizontal(|ui| {
                    for tab in [ActiveTab::Monitor, ActiveTab::KnownDevices] {
                        let selected = new_tab == tab;
                        let (fill, text_color, stroke) = if selected {
                            (tc.bg_deep, tc.accent, egui::Stroke::new(1.0, tc.accent))
                        } else {
                            (
                                tc.bg_elevated,
                                tc.text_sec,
                                egui::Stroke::new(0.5, tc.border),
                            )
                        };

                        let btn = egui::Button::new(
                            egui::RichText::new(tab.label())
                                .size(13.0)
                                .color(text_color)
                                .strong(),
                        )
                        .fill(fill)
                        .stroke(stroke)
                        .rounding(egui::Rounding {
                            nw: 6.0,
                            ne: 6.0,
                            sw: 0.0,
                            se: 0.0,
                        });

                        if ui.add(btn).clicked() && !selected {
                            new_tab = tab;
                        }
                    }

                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        ui.label(
                            egui::RichText::new(format!(
                                "{} known ({} online)",
                                known_total, known_online
                            ))
                            .color(tc.text_muted)
                            .size(12.0),
                        );
                    });
                });
            });

        if new_tab != self.active_tab {
            self.active_tab = new_tab;
            self.save_prefs();
        }

        // ── Footer ──
        egui::TopBottomPanel::bottom("footer")
            .frame(
                egui::Frame::none()
                    .fill(tc.bg_surface)
                    .inner_margin(egui::Margin::symmetric(14.0, 7.0))
                    .stroke(egui::Stroke::new(1.0, tc.border)),
            )
            .show(ctx, |ui| {
                ui.horizontal(|ui| {
                    ui.label(
                        egui::RichText::new(format!("{} events logged", events.len()))
                            .color(tc.text_muted)
                            .size(12.0),
                    );
                    ui.label(egui::RichText::new("|").color(tc.accent).size(10.0));
                    ui.label(
                        egui::RichText::new(format!(
                            "{} known ({} online)",
                            known_total, known_online
                        ))
                        .color(tc.text_muted)
                        .size(12.0),
                    );
                    ui.label(egui::RichText::new("|").color(tc.accent).size(10.0));
                    ui.label(
                        egui::RichText::new("Log: device-history.log")
                            .color(tc.text_muted)
                            .size(12.0),
                    );
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        ui.label(
                            egui::RichText::new("tront.xyz")
                                .color(tc.text_muted)
                                .size(11.0),
                        );
                    });
                });
            });

        // ── Central panel ──
        let mut about_open = self.show_about;
        let mut search_query = self.search_query.clone();
        let mut sort_mode = self.sort_mode;
        let mut sort_ascending = self.sort_ascending;
        let mut selected_device = self.selected_device.clone();
        let mut nickname_buf = self.nickname_buf.clone();

        egui::CentralPanel::default()
            .frame(
                egui::Frame::none()
                    .fill(tc.bg_deep)
                    .inner_margin(egui::Margin::symmetric(14.0, 10.0)),
            )
            .show(ctx, |ui| {
                ui.spacing_mut().item_spacing.y = 4.0;

                match self.active_tab {
                    ActiveTab::Monitor => {
                        // ── About section ──
                        let about_id = ui.make_persistent_id("about_section");
                        let about_state =
                            egui::collapsing_header::CollapsingState::load_with_default_open(
                                ctx, about_id, about_open,
                            );
                        let was_open = about_state.is_open();
                        about_state
                            .show_header(ui, |ui| {
                                ui.label(
                                    egui::RichText::new("About")
                                        .size(13.0)
                                        .color(tc.text_sec),
                                );
                            })
                            .body(|ui| {
                                let about_frame = egui::Frame::none()
                                    .fill(tc.bg_surface)
                                    .rounding(6.0)
                                    .stroke(egui::Stroke::new(0.5, tc.border))
                                    .inner_margin(egui::Margin::same(12.0));

                                about_frame.show(ui, |ui: &mut egui::Ui| {
                                    ui.spacing_mut().item_spacing.y = 6.0;

                                    ui.label(
                                        egui::RichText::new("WTF just disconnected?")
                                            .size(16.0)
                                            .strong()
                                            .color(tc.accent),
                                    );
                                    ui.add_space(2.0);
                                    ui.label(
                                        egui::RichText::new(
                                            "Real-time USB device monitor for Windows. Watches for \
                                             connect/disconnect events via WMI polling and logs everything \
                                             to device-history.log with timestamps.",
                                        )
                                        .size(13.0)
                                        .color(tc.text_sec),
                                    );
                                    ui.add_space(4.0);

                                    draw_rainbow_separator(ui, &tc);

                                    ui.add_space(4.0);

                                    let features = [
                                        (tc.green, "Live monitoring", "500ms poll interval, instant detection"),
                                        (tc.cyan, "Event log", "Timestamped connect/disconnect history"),
                                        (tc.yellow, "Device details", "VID:PID, class, manufacturer for every device"),
                                        (tc.orange, "File logging", "Persistent log at device-history.log"),
                                        (tc.accent, "Known devices", "Persistent cache of every device ever seen"),
                                        (tc.pink, "CLI mode", "Run with --cli for terminal output"),
                                    ];

                                    for (color, title, desc) in &features {
                                        ui.horizontal(|ui| {
                                            ui.label(
                                                egui::RichText::new("|")
                                                    .color(*color)
                                                    .size(12.0),
                                            );
                                            ui.label(
                                                egui::RichText::new(*title)
                                                    .color(tc.text)
                                                    .strong()
                                                    .size(12.0),
                                            );
                                            ui.label(
                                                egui::RichText::new(*desc)
                                                    .color(tc.text_sec)
                                                    .size(12.0),
                                            );
                                        });
                                    }
                                });
                            });

                        let is_open =
                            egui::collapsing_header::CollapsingState::load_with_default_open(
                                ctx, about_id, about_open,
                            )
                            .is_open();
                        if is_open != was_open {
                            about_open = is_open;
                        }

                        ui.add_space(6.0);

                        // ── Event Log section ──
                        let events_empty = events.is_empty();
                        let half_height = (ui.available_height() - 30.0) * 0.45;

                        ui.horizontal(|ui| {
                            ui.label(
                                egui::RichText::new(format!("Event Log ({})", events.len()))
                                    .size(15.0)
                                    .color(tc.cyan)
                                    .strong(),
                            );
                            ui.with_layout(
                                egui::Layout::right_to_left(egui::Align::Center),
                                |ui| {
                                    let clear_btn = egui::Button::new(
                                        egui::RichText::new("Clear")
                                            .color(tc.orange)
                                            .size(12.0),
                                    )
                                    .stroke(egui::Stroke::new(1.0, tc.orange))
                                    .fill(egui::Color32::TRANSPARENT)
                                    .rounding(4.0);

                                    if ui.add(clear_btn).clicked() {
                                        if let Ok(mut s) = state_arc.lock() {
                                            s.events.clear();
                                        }
                                    }
                                },
                            );
                        });

                        ui.add_space(4.0);

                        egui::Frame::none()
                            .fill(tc.bg_surface)
                            .rounding(6.0)
                            .stroke(egui::Stroke::new(1.0, tc.border))
                            .inner_margin(egui::Margin::same(6.0))
                            .show(ui, |ui: &mut egui::Ui| {
                                ui.set_width(ui.available_width());
                                egui::ScrollArea::vertical()
                                    .id_salt("event_log")
                                    .max_height(if events_empty {
                                        60.0
                                    } else {
                                        half_height.max(80.0)
                                    })
                                    .stick_to_bottom(true)
                                    .show(ui, |ui| {
                                        ui.spacing_mut().item_spacing.y = 3.0;

                                        if events_empty {
                                            ui.add_space(16.0);
                                            ui.vertical_centered(|ui| {
                                                ui.label(
                                                    egui::RichText::new(
                                                        "No events yet -- waiting for USB changes...",
                                                    )
                                                    .color(tc.text_sec)
                                                    .italics()
                                                    .size(13.0),
                                                );
                                            });
                                        } else {
                                            for (ev_idx, event) in events.iter().enumerate() {
                                                let is_selected = selected_device.as_deref() == Some(&event.device_id);
                                                let (accent, icon, label) = match event.kind {
                                                    EventKind::Connect => {
                                                        (tc.green, "^", "CONNECT")
                                                    }
                                                    EventKind::Disconnect => {
                                                        (tc.red, "v", "DISCONNECT")
                                                    }
                                                };

                                                let card_fill = if is_selected {
                                                    blend(tc.bg_elevated, tc.accent, 0.12)
                                                } else {
                                                    blend(tc.bg_elevated, accent, 0.05)
                                                };
                                                let card_stroke = if is_selected {
                                                    egui::Stroke::new(1.5, tc.accent)
                                                } else {
                                                    egui::Stroke::new(0.5, tc.border)
                                                };

                                                let frame_resp = egui::Frame::none()
                                                    .fill(card_fill)
                                                    .rounding(4.0)
                                                    .stroke(card_stroke)
                                                    .inner_margin(egui::Margin {
                                                        left: 10.0,
                                                        right: 8.0,
                                                        top: 4.0,
                                                        bottom: 4.0,
                                                    })
                                                    .show(ui, |ui: &mut egui::Ui| {
                                                        ui.set_width(ui.available_width());
                                                        let rect = ui.max_rect();

                                                        ui.painter().rect_filled(
                                                            egui::Rect::from_min_size(
                                                                rect.left_top(),
                                                                egui::Vec2::new(
                                                                    3.0,
                                                                    rect.height(),
                                                                ),
                                                            ),
                                                            egui::Rounding {
                                                                nw: 4.0,
                                                                sw: 4.0,
                                                                ne: 0.0,
                                                                se: 0.0,
                                                            },
                                                            accent,
                                                        );

                                                        ui.horizontal(|ui| {
                                                            ui.spacing_mut().item_spacing.x = 6.0;

                                                            ui.label(
                                                                egui::RichText::new(
                                                                    &event.timestamp,
                                                                )
                                                                .color(tc.text_sec)
                                                                .monospace()
                                                                .size(12.0),
                                                            );
                                                            ui.label(
                                                                egui::RichText::new(format!(
                                                                    "{} {}",
                                                                    icon, label
                                                                ))
                                                                .color(accent)
                                                                .strong()
                                                                .monospace()
                                                                .size(12.0),
                                                            );
                                                            ui.add(
                                                                egui::Label::new(
                                                                    egui::RichText::new(
                                                                        &event.name,
                                                                    )
                                                                    .color(tc.text)
                                                                    .size(12.0),
                                                                )
                                                                .truncate(),
                                                            );
                                                            if let Some(vp) = &event.vid_pid {
                                                                ui.label(
                                                                    egui::RichText::new(format!(
                                                                        "[{}]",
                                                                        vp
                                                                    ))
                                                                    .color(tc.yellow)
                                                                    .monospace()
                                                                    .size(11.0),
                                                                );
                                                            }
                                                            // Drive letter badge
                                                            if let Some(si) = storage_info.get(&event.device_id) {
                                                                for vol in &si.volumes {
                                                                    ui.label(
                                                                        egui::RichText::new(format!("[{}]", vol.drive_letter))
                                                                            .color(tc.green)
                                                                            .strong()
                                                                            .monospace()
                                                                            .size(11.0),
                                                                    );
                                                                }
                                                            }
                                                            ui.label(
                                                                egui::RichText::new(&event.class)
                                                                    .color(tc.accent)
                                                                    .monospace()
                                                                    .size(10.0),
                                                            );
                                                            if let Some(mfr) = &event.manufacturer
                                                            {
                                                                ui.add(
                                                                    egui::Label::new(
                                                                        egui::RichText::new(
                                                                            format!("({})", mfr),
                                                                        )
                                                                        .color(tc.text_sec)
                                                                        .size(11.0),
                                                                    )
                                                                    .truncate(),
                                                                );
                                                            }
                                                        });
                                                    });

                                                // Click interaction
                                                let click_resp = ui.interact(
                                                    frame_resp.response.rect,
                                                    egui::Id::new("event_click").with(ev_idx),
                                                    egui::Sense::click(),
                                                );
                                                if click_resp.clicked() {
                                                    if is_selected {
                                                        selected_device = None;
                                                    } else {
                                                        selected_device = Some(event.device_id.clone());
                                                    }
                                                }
                                                if click_resp.hovered() {
                                                    ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand);
                                                }
                                            }
                                        }
                                    });
                            });

                        ui.add_space(8.0);
                        draw_rainbow_separator(ui, &tc);
                        ui.add_space(6.0);

                        // ── Connected Devices section ──
                        ui.horizontal(|ui| {
                            ui.label(
                                egui::RichText::new(format!(
                                    "Connected Devices ({})",
                                    devices.len()
                                ))
                                .size(15.0)
                                .color(tc.cyan)
                                .strong(),
                            );
                        });

                        ui.add_space(4.0);

                        egui::Frame::none()
                            .fill(tc.bg_surface)
                            .rounding(6.0)
                            .stroke(egui::Stroke::new(1.0, tc.border))
                            .inner_margin(egui::Margin::same(6.0))
                            .show(ui, |ui: &mut egui::Ui| {
                                ui.set_width(ui.available_width());
                                let remaining = ui.available_height().max(60.0);
                                egui::ScrollArea::vertical()
                                    .id_salt("devices_list")
                                    .max_height(remaining)
                                    .show(ui, |ui| {
                                        ui.spacing_mut().item_spacing.y = 2.0;

                                        for (dev_idx, (dev_id, dev)) in devices.iter().enumerate() {
                                            let is_selected = selected_device.as_deref() == Some(dev_id.as_str());
                                            let card_fill = if is_selected {
                                                blend(tc.bg_elevated, tc.accent, 0.12)
                                            } else {
                                                tc.bg_elevated
                                            };
                                            let card_stroke = if is_selected {
                                                egui::Stroke::new(1.5, tc.accent)
                                            } else {
                                                egui::Stroke::new(0.5, tc.border)
                                            };

                                            let frame_resp = egui::Frame::none()
                                                .fill(card_fill)
                                                .rounding(4.0)
                                                .stroke(card_stroke)
                                                .inner_margin(egui::Margin::symmetric(10.0, 4.0))
                                                .show(ui, |ui: &mut egui::Ui| {
                                                    ui.set_width(ui.available_width());
                                                    ui.horizontal(|ui| {
                                                        ui.spacing_mut().item_spacing.x = 6.0;

                                                        // Drive info FIRST if storage device
                                                        let conn_si = storage_info.get(dev_id);
                                                        if let Some(si) = conn_si {
                                                            for vol in &si.volumes {
                                                                ui.label(
                                                                    egui::RichText::new(&vol.drive_letter)
                                                                        .color(tc.green)
                                                                        .strong()
                                                                        .monospace()
                                                                        .size(13.0),
                                                                );
                                                                if !vol.volume_name.is_empty() {
                                                                    ui.label(
                                                                        egui::RichText::new(format!("\"{}\"", vol.volume_name))
                                                                            .color(tc.text)
                                                                            .strong()
                                                                            .size(12.0),
                                                                    );
                                                                }
                                                            }
                                                            if !si.model.is_empty() {
                                                                ui.label(
                                                                    egui::RichText::new(&si.model)
                                                                        .color(tc.text_sec)
                                                                        .size(11.0),
                                                                );
                                                            }
                                                        } else {
                                                            ui.label(
                                                                egui::RichText::new(dev.class())
                                                                    .color(tc.accent)
                                                                    .monospace()
                                                                    .size(11.0),
                                                            );
                                                            ui.add(
                                                                egui::Label::new(
                                                                    egui::RichText::new(
                                                                        dev.display_name(),
                                                                    )
                                                                    .color(tc.text)
                                                                    .size(12.0),
                                                                )
                                                                .truncate(),
                                                            );
                                                        }
                                                        // Nickname in teal
                                                        if let Some(kd) = known_devices.devices.get(dev_id) {
                                                            if let Some(nick) = &kd.nickname {
                                                                if !nick.is_empty() {
                                                                    ui.label(
                                                                        egui::RichText::new(format!("({})", nick))
                                                                            .color(tc.teal)
                                                                            .size(11.0),
                                                                    );
                                                                }
                                                            }
                                                        }
                                                        if let Some(vp) = dev.vid_pid() {
                                                            ui.label(
                                                                egui::RichText::new(format!(
                                                                    "[{}]",
                                                                    vp
                                                                ))
                                                                .color(tc.yellow)
                                                                .monospace()
                                                                .size(10.0),
                                                            );
                                                        }
                                                        if conn_si.is_none() {
                                                            if let Some(mfr) = &dev.Manufacturer {
                                                                ui.add(
                                                                    egui::Label::new(
                                                                        egui::RichText::new(mfr)
                                                                            .color(tc.text_sec)
                                                                            .size(10.0),
                                                                    )
                                                                    .truncate(),
                                                                );
                                                            }
                                                        }
                                                    });
                                                });

                                            // Click interaction
                                            let click_resp = ui.interact(
                                                frame_resp.response.rect,
                                                egui::Id::new("connected_click").with(dev_idx),
                                                egui::Sense::click(),
                                            );
                                            if click_resp.clicked() {
                                                if is_selected {
                                                    selected_device = None;
                                                } else {
                                                    selected_device = Some(dev_id.clone());
                                                    if let Some(kd) = known_devices.devices.get(dev_id) {
                                                        nickname_buf = kd.nickname.clone().unwrap_or_default();
                                                    }
                                                }
                                            }
                                            if click_resp.hovered() {
                                                ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand);
                                            }

                                            // Inline detail panel
                                            if is_selected {
                                                let dev_si = storage_info.get(dev_id)
                                                    .or_else(|| known_devices.devices.get(dev_id).and_then(|kd| kd.storage_info.as_ref()));
                                                let kd = known_devices.devices.get(dev_id);
                                                draw_device_detail_panel(
                                                    ui, &tc, dev_id, dev.display_name(),
                                                    dev.vid_pid(), dev.class(),
                                                    dev.Manufacturer.as_deref(),
                                                    dev.Description.as_deref(),
                                                    kd, dev_si,
                                                    &mut nickname_buf,
                                                    &state_arc,
                                                    true, // is_connected
                                                );
                                            }
                                        }
                                    });
                            });
                    }

                    ActiveTab::KnownDevices => {
                        // ── Search bar ──
                        ui.horizontal(|ui| {
                            ui.label(
                                egui::RichText::new("Search:")
                                    .size(14.0)
                                    .color(tc.text_sec),
                            );
                            let te = egui::TextEdit::singleline(&mut search_query)
                                .hint_text("Search by name, class, manufacturer, VID:PID...")
                                .desired_width(300.0)
                                .text_color(tc.text);
                            ui.add(te);

                            ui.add_space(12.0);

                            // Sort buttons
                            for mode in [
                                SortMode::Status,
                                SortMode::Name,
                                SortMode::LastSeen,
                                SortMode::TimesSeen,
                                SortMode::FirstSeen,
                            ] {
                                let selected = sort_mode == mode;
                                let arrow = if selected {
                                    if sort_ascending {
                                        " ^"
                                    } else {
                                        " v"
                                    }
                                } else {
                                    ""
                                };
                                let label_color = if selected { tc.accent } else { tc.text_sec };
                                let btn = egui::Button::new(
                                    egui::RichText::new(format!("{}{}", mode.label(), arrow))
                                        .size(11.0)
                                        .color(label_color),
                                )
                                .fill(if selected {
                                    blend(tc.bg_elevated, tc.accent, 0.12)
                                } else {
                                    egui::Color32::TRANSPARENT
                                })
                                .stroke(if selected {
                                    egui::Stroke::new(1.0, tc.accent)
                                } else {
                                    egui::Stroke::new(0.5, tc.border)
                                })
                                .rounding(3.0);

                                if ui.add(btn).clicked() {
                                    if selected {
                                        sort_ascending = !sort_ascending;
                                    } else {
                                        sort_mode = mode;
                                        sort_ascending = true;
                                    }
                                }
                            }
                        });

                        ui.add_space(6.0);

                        // ── Filter + sort devices ──
                        let query_lower = search_query.to_lowercase();
                        let mut filtered: Vec<&KnownDevice> = known_devices
                            .devices
                            .values()
                            .filter(|d| {
                                if query_lower.is_empty() {
                                    return true;
                                }
                                d.name.to_lowercase().contains(&query_lower)
                                    || d.device_id.to_lowercase().contains(&query_lower)
                                    || d.class.to_lowercase().contains(&query_lower)
                                    || d.manufacturer.to_lowercase().contains(&query_lower)
                                    || d.vid_pid.to_lowercase().contains(&query_lower)
                                    || d.nickname.as_deref().unwrap_or("").to_lowercase().contains(&query_lower)
                            })
                            .collect();

                        filtered.sort_by(|a, b| {
                            let cmp = match sort_mode {
                                SortMode::Status => a
                                    .currently_connected
                                    .cmp(&b.currently_connected)
                                    .then_with(|| {
                                        a.name.to_lowercase().cmp(&b.name.to_lowercase())
                                    }),
                                SortMode::Name => {
                                    a.name.to_lowercase().cmp(&b.name.to_lowercase())
                                }
                                SortMode::LastSeen => a.last_seen.cmp(&b.last_seen),
                                SortMode::TimesSeen => a.times_seen.cmp(&b.times_seen),
                                SortMode::FirstSeen => a.first_seen.cmp(&b.first_seen),
                            };
                            if sort_ascending {
                                cmp
                            } else {
                                cmp.reverse()
                            }
                        });

                        // ── Device cards ──
                        egui::Frame::none()
                            .fill(tc.bg_surface)
                            .rounding(6.0)
                            .stroke(egui::Stroke::new(1.0, tc.border))
                            .inner_margin(egui::Margin::same(6.0))
                            .show(ui, |ui: &mut egui::Ui| {
                                ui.set_width(ui.available_width());
                                let remaining = ui.available_height().max(60.0);
                                egui::ScrollArea::vertical()
                                    .id_salt("known_devices_list")
                                    .max_height(remaining)
                                    .show(ui, |ui| {
                                        ui.spacing_mut().item_spacing.y = 3.0;

                                        if known_devices.devices.is_empty() {
                                            ui.add_space(24.0);
                                            ui.vertical_centered(|ui| {
                                                ui.label(
                                                    egui::RichText::new(
                                                        "No devices seen yet -- plug in a USB device to get started",
                                                    )
                                                    .color(tc.text_sec)
                                                    .italics()
                                                    .size(13.0),
                                                );
                                            });
                                        } else if filtered.is_empty() {
                                            ui.add_space(24.0);
                                            ui.vertical_centered(|ui| {
                                                ui.label(
                                                    egui::RichText::new(format!(
                                                        "No devices matching '{}'",
                                                        search_query
                                                    ))
                                                    .color(tc.text_sec)
                                                    .italics()
                                                    .size(13.0),
                                                );
                                            });
                                        } else {
                                            let forget_id: Option<String> = None;

                                            for (kd_idx, dev) in filtered.iter().enumerate() {
                                                let is_selected = selected_device.as_deref() == Some(&dev.device_id);
                                                let card_fill = if is_selected {
                                                    blend(tc.bg_elevated, tc.accent, 0.10)
                                                } else if dev.currently_connected {
                                                    blend(tc.bg_elevated, tc.green, 0.06)
                                                } else {
                                                    tc.bg_elevated
                                                };
                                                let card_stroke = if is_selected {
                                                    egui::Stroke::new(1.5, tc.accent)
                                                } else {
                                                    egui::Stroke::new(0.5, tc.border)
                                                };

                                                let frame_resp = egui::Frame::none()
                                                    .fill(card_fill)
                                                    .rounding(4.0)
                                                    .stroke(card_stroke)
                                                    .inner_margin(egui::Margin {
                                                        left: 10.0,
                                                        right: 8.0,
                                                        top: 5.0,
                                                        bottom: 5.0,
                                                    })
                                                    .show(ui, |ui: &mut egui::Ui| {
                                                        ui.set_width(ui.available_width());

                                                        // Row 1: status dot, class, name, nickname, vid:pid, drive letter, manufacturer
                                                        ui.horizontal(|ui| {
                                                            ui.spacing_mut().item_spacing.x = 6.0;

                                                            let dot_color =
                                                                if dev.currently_connected {
                                                                    tc.green
                                                                } else {
                                                                    tc.text_muted
                                                                };
                                                            ui.label(
                                                                egui::RichText::new("*")
                                                                    .color(dot_color)
                                                                    .size(10.0),
                                                            );
                                                            // Drive letter + volume name FIRST if storage device
                                                            let dev_si = storage_info.get(&dev.device_id)
                                                                .or(dev.storage_info.as_ref());
                                                            if let Some(si) = dev_si {
                                                                for vol in &si.volumes {
                                                                    ui.label(
                                                                        egui::RichText::new(&vol.drive_letter)
                                                                            .color(tc.green)
                                                                            .strong()
                                                                            .monospace()
                                                                            .size(13.0),
                                                                    );
                                                                    if !vol.volume_name.is_empty() {
                                                                        ui.label(
                                                                            egui::RichText::new(format!("\"{}\"", vol.volume_name))
                                                                                .color(tc.text)
                                                                                .strong()
                                                                                .size(12.0),
                                                                        );
                                                                    }
                                                                }
                                                                if !si.model.is_empty() {
                                                                    ui.label(
                                                                        egui::RichText::new(&si.model)
                                                                            .color(tc.text_sec)
                                                                            .size(11.0),
                                                                    );
                                                                }
                                                            } else {
                                                                // Non-storage: show class + name as before
                                                                ui.label(
                                                                    egui::RichText::new(&dev.class)
                                                                        .color(tc.accent)
                                                                        .monospace()
                                                                        .size(11.0),
                                                                );
                                                                ui.add(
                                                                    egui::Label::new(
                                                                        egui::RichText::new(&dev.name)
                                                                            .color(tc.text)
                                                                            .size(12.0),
                                                                    )
                                                                    .truncate(),
                                                                );
                                                            }
                                                            // Nickname in teal
                                                            if let Some(nick) = &dev.nickname {
                                                                if !nick.is_empty() {
                                                                    ui.label(
                                                                        egui::RichText::new(format!("({})", nick))
                                                                            .color(tc.teal)
                                                                            .size(11.0),
                                                                    );
                                                                }
                                                            }
                                                            if !dev.vid_pid.is_empty() {
                                                                ui.label(
                                                                    egui::RichText::new(format!(
                                                                        "[{}]",
                                                                        dev.vid_pid
                                                                    ))
                                                                    .color(tc.yellow)
                                                                    .monospace()
                                                                    .size(10.0),
                                                                );
                                                            }
                                                            if dev_si.is_none() && !dev.manufacturer.is_empty() {
                                                                ui.add(
                                                                    egui::Label::new(
                                                                        egui::RichText::new(
                                                                            &dev.manufacturer,
                                                                        )
                                                                        .color(tc.text_sec)
                                                                        .size(10.0),
                                                                    )
                                                                    .truncate(),
                                                                );
                                                            }
                                                        });

                                                        // Row 2: timestamps, times seen (no buttons — moved to detail panel)
                                                        ui.horizontal(|ui| {
                                                            ui.spacing_mut().item_spacing.x = 12.0;
                                                            ui.label(
                                                                egui::RichText::new(format!(
                                                                    "First: {}",
                                                                    dev.first_seen
                                                                ))
                                                                .color(tc.text_muted)
                                                                .size(10.0),
                                                            );
                                                            ui.label(
                                                                egui::RichText::new(format!(
                                                                    "Last: {}",
                                                                    dev.last_seen
                                                                ))
                                                                .color(tc.text_muted)
                                                                .size(10.0),
                                                            );
                                                            ui.label(
                                                                egui::RichText::new(format!(
                                                                    "Seen {}x",
                                                                    dev.times_seen
                                                                ))
                                                                .color(tc.teal)
                                                                .size(10.0),
                                                            );
                                                        });
                                                    });

                                                // Click interaction
                                                let click_resp = ui.interact(
                                                    frame_resp.response.rect,
                                                    egui::Id::new("known_click").with(kd_idx),
                                                    egui::Sense::click(),
                                                );
                                                if click_resp.clicked() {
                                                    if is_selected {
                                                        selected_device = None;
                                                    } else {
                                                        selected_device = Some(dev.device_id.clone());
                                                        nickname_buf = dev.nickname.clone().unwrap_or_default();
                                                    }
                                                }
                                                if click_resp.hovered() {
                                                    ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand);
                                                }

                                                // Inline detail panel
                                                if is_selected {
                                                    let dev_si = storage_info.get(&dev.device_id)
                                                        .or(dev.storage_info.as_ref());
                                                    let vid_pid_opt = if dev.vid_pid.is_empty() { None } else { Some(dev.vid_pid.clone()) };
                                                    draw_device_detail_panel(
                                                        ui, &tc, &dev.device_id, &dev.name,
                                                        vid_pid_opt, &dev.class,
                                                        Some(dev.manufacturer.as_str()).filter(|s| !s.is_empty()),
                                                        Some(dev.description.as_str()).filter(|s| !s.is_empty()),
                                                        Some(dev), dev_si,
                                                        &mut nickname_buf,
                                                        &state_arc,
                                                        dev.currently_connected,
                                                    );
                                                    // Check if forget was requested in detail panel
                                                    // (handled inside draw_device_detail_panel via state_arc)
                                                }
                                            }

                                            // Process forget action
                                            if let Some(id) = forget_id {
                                                if let Ok(mut s) = state_arc.lock() {
                                                    s.known_devices.devices.remove(&id);
                                                    save_cache(&s.known_devices);
                                                }
                                            }
                                        }
                                    });
                            });
                    }
                }
            });

        // Write back changed values
        if about_open != self.show_about {
            self.show_about = about_open;
            self.save_prefs();
        }
        self.search_query = search_query;
        self.sort_mode = sort_mode;
        self.sort_ascending = sort_ascending;
        self.selected_device = selected_device;
        self.nickname_buf = nickname_buf;
    }
}

// ── CLI mode ───────────────────────────────────────────────────

fn run_cli() {
    // Attach to parent console (or allocate one) when windows_subsystem = "windows"
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

    use colored::*;

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

// ── Entry point ────────────────────────────────────────────────

fn main() {
    let args: Vec<String> = std::env::args().collect();

    if args.iter().any(|a| a == "--cli") {
        run_cli();
        return;
    }

    let cache = load_cache();

    let state = Arc::new(Mutex::new(AppState {
        devices: Vec::new(),
        events: Vec::new(),
        error: None,
        known_devices: cache,
        storage_info: HashMap::new(),
    }));

    let state_bg = state.clone();
    thread::spawn(move || monitor_loop(state_bg));

    let icon = load_icon();

    // ── Tray icon setup ──
    let show_item = MenuItem::new("Show", true, None);
    let hide_item = MenuItem::new("Hide", true, None);
    let exit_item = MenuItem::new("Exit", true, None);

    let tray_menu_ids = TrayMenuIds {
        show: show_item.id().clone(),
        hide: hide_item.id().clone(),
        exit: exit_item.id().clone(),
    };

    let tray_menu = Menu::new();
    let _ = tray_menu.append(&show_item);
    let _ = tray_menu.append(&hide_item);
    let _ = tray_menu.append(&PredefinedMenuItem::separator());
    let _ = tray_menu.append(&exit_item);

    let tray_icon_image = {
        let png_bytes = include_bytes!("../assets/icon.png");
        let img = image::load_from_memory(png_bytes)
            .expect("Failed to load tray icon")
            .into_rgba8();
        let (w, h) = img.dimensions();
        tray_icon::Icon::from_rgba(img.into_raw(), w, h).expect("Failed to create tray icon")
    };

    let _tray = TrayIconBuilder::new()
        .with_menu(Box::new(tray_menu))
        .with_tooltip("Device History")
        .with_icon(tray_icon_image)
        .build()
        .expect("Failed to create tray icon");

    let mut viewport = egui::ViewportBuilder::default()
        .with_inner_size([720.0, 620.0])
        .with_min_inner_size([420.0, 340.0])
        .with_title("Device History");

    if let Some(icon_data) = icon {
        viewport = viewport.with_icon(std::sync::Arc::new(icon_data));
    }

    let options = eframe::NativeOptions {
        viewport,
        ..Default::default()
    };

    if let Err(e) = eframe::run_native(
        "Device History",
        options,
        Box::new(move |cc| {
            // Tray event thread — handles show/hide/exit independently of eframe's render loop.
            // eframe may skip update() for hidden windows, so we poll tray events here instead.
            let ctx = cc.egui_ctx.clone();
            let show_id = tray_menu_ids.show.clone();
            let hide_id = tray_menu_ids.hide.clone();
            let exit_id = tray_menu_ids.exit.clone();
            thread::spawn(move || loop {
                thread::sleep(Duration::from_millis(100));

                if let Ok(event) = MenuEvent::receiver().try_recv() {
                    if event.id == show_id {
                        #[cfg(windows)]
                        win32::show_window();
                        ctx.request_repaint();
                    } else if event.id == hide_id {
                        #[cfg(windows)]
                        win32::hide_window();
                    } else if event.id == exit_id {
                        std::process::exit(0);
                    }
                }

                if let Ok(TrayIconEvent::DoubleClick { .. }) = TrayIconEvent::receiver().try_recv() {
                    #[cfg(windows)]
                    win32::show_window();
                    ctx.request_repaint();
                }
            });
            Ok(Box::new(DeviceHistoryApp::new(state, tray_menu_ids)))
        }),
    ) {
        eprintln!("GUI error: {}", e);
    }
}
