// Hide console window in GUI mode (release builds only)
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use chrono::Local;
use iced::widget::{
    button, column, container, horizontal_rule, horizontal_space, progress_bar, row, scrollable,
    text, text_input, Column, Row, Space,
};
use iced::{
    color, Element, Length, Subscription, Task as IcedTask, Theme,
};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fs::OpenOptions;
use std::io::Write as IoWrite;
use std::sync::{mpsc, Arc, Mutex};
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
    let usb_serial = device_id.rsplit('\\').next()?.to_uppercase();
    if usb_serial.is_empty() {
        return None;
    }

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

    let matched = drives.iter().find(|d| {
        if let Some(serial) = &d.SerialNumber {
            let s = serial.trim().replace(' ', "").to_uppercase();
            if !s.is_empty() && (s.contains(&usb_serial) || usb_serial.contains(&s)) {
                return true;
            }
        }
        if let Some(pnp) = &d.PNPDeviceID {
            let p = pnp.to_uppercase();
            if p.contains(&usb_serial) {
                return true;
            }
        }
        false
    });

    if matched.is_none() {
        log_to_file(&format!(
            "ENRICH FAIL: no drive matched usb_serial={}",
            usb_serial
        ));
        return None;
    }
    let matched = matched?;

    let volumes = query_volumes_for_drive(matched);
    log_to_file(&format!(
        "ENRICH: matched drive={} serial={} → {} volumes [{}]",
        matched.Model.as_deref().unwrap_or("?"),
        matched.SerialNumber.as_deref().unwrap_or("?").trim(),
        volumes.len(),
        volumes
            .iter()
            .map(|v| v.drive_letter.as_str())
            .collect::<Vec<_>>()
            .join(", ")
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

// AppState was used by the old egui Arc<Mutex> approach; now replaced by MonitorSnapshot channel

// ── Preferences ────────────────────────────────────────────────

const PREFS_FILE: &str = "device-history.prefs";

struct Prefs {
    theme: String,
    active_tab: String,
}

impl Prefs {
    fn load() -> Self {
        let defaults = Self {
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
            "theme={}\nactive_tab={}\n",
            self.theme, self.active_tab
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

/// Snapshot sent from monitor thread to UI
#[derive(Clone)]
struct MonitorSnapshot {
    devices: Vec<(String, UsbDevice)>,
    events: Vec<DeviceEvent>,
    known_devices: KnownDeviceCache,
    storage_info: HashMap<String, StorageInfo>,
    error: Option<String>,
}

fn monitor_loop(tx: mpsc::Sender<MonitorSnapshot>) {
    let com = match COMLibrary::new() {
        Ok(c) => c,
        Err(e) => {
            let _ = tx.send(MonitorSnapshot {
                devices: vec![],
                events: vec![],
                known_devices: KnownDeviceCache::new(),
                storage_info: HashMap::new(),
                error: Some(format!("COM init failed: {}", e)),
            });
            return;
        }
    };
    let wmi = match WMIConnection::new(com) {
        Ok(w) => w,
        Err(e) => {
            let _ = tx.send(MonitorSnapshot {
                devices: vec![],
                events: vec![],
                known_devices: KnownDeviceCache::new(),
                storage_info: HashMap::new(),
                error: Some(format!("WMI connect failed: {}", e)),
            });
            return;
        }
    };

    let mut prev = match query_devices(&wmi) {
        Some(d) => d,
        None => {
            let _ = tx.send(MonitorSnapshot {
                devices: vec![],
                events: vec![],
                known_devices: KnownDeviceCache::new(),
                storage_info: HashMap::new(),
                error: Some("Failed to query USB devices".into()),
            });
            return;
        }
    };

    let mut known_devices = load_cache();
    let mut storage_info: HashMap<String, StorageInfo> = HashMap::new();
    let mut all_events: Vec<DeviceEvent> = Vec::new();

    // Initial snapshot — merge into cache
    {
        let now = Local::now().format("%Y-%m-%d %H:%M:%S").to_string();
        for dev in known_devices.devices.values_mut() {
            dev.currently_connected = false;
        }
        for (id, dev) in &prev {
            let is_new = !known_devices.devices.contains_key(id);
            let entry = known_devices
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
        save_cache(&known_devices);
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
                storage_info.insert(id.clone(), info.clone());
                if let Some(kd) = known_devices.devices.get_mut(id) {
                    kd.storage_info = Some(info);
                }
                save_cache(&known_devices);
            }
        }
    }

    // Send initial snapshot
    let mut sorted: Vec<_> = prev.iter().map(|(id, d)| (id.clone(), d.clone())).collect();
    sorted.sort_by(|a, b| {
        a.1.display_name()
            .to_lowercase()
            .cmp(&b.1.display_name().to_lowercase())
    });
    let _ = tx.send(MonitorSnapshot {
        devices: sorted,
        events: all_events.clone(),
        known_devices: known_devices.clone(),
        storage_info: storage_info.clone(),
        error: None,
    });

    log_to_file(&format!("Started monitoring — {} devices", prev.len()));

    let mut pending_enrichments: Vec<(String, Instant)> = Vec::new();

    loop {
        thread::sleep(Duration::from_millis(500));

        // Process pending enrichments (2s delay for drives to mount)
        let now_instant = Instant::now();
        let ready: Vec<String> = pending_enrichments
            .iter()
            .filter(|(_, scheduled)| {
                now_instant.duration_since(*scheduled) >= Duration::from_secs(2)
            })
            .map(|(id, _)| id.clone())
            .collect();
        pending_enrichments.retain(|(_, scheduled)| {
            now_instant.duration_since(*scheduled) < Duration::from_secs(2)
        });
        let mut enriched = false;
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
                storage_info.insert(enrich_id.clone(), info.clone());
                if let Some(kd) = known_devices.devices.get_mut(&enrich_id) {
                    kd.storage_info = Some(info);
                }
                save_cache(&known_devices);
                enriched = true;
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

            for event in &new_events {
                match event.kind {
                    EventKind::Connect => {
                        if let Some(dev) = current.get(&event.device_id) {
                            let is_new =
                                !known_devices.devices.contains_key(&event.device_id);
                            let entry = known_devices
                                .devices
                                .entry(event.device_id.clone())
                                .or_insert_with(|| KnownDevice {
                                    device_id: event.device_id.clone(),
                                    name: dev.display_name().to_string(),
                                    vid_pid: dev.vid_pid().unwrap_or_default(),
                                    class: dev.class().to_string(),
                                    manufacturer: dev
                                        .Manufacturer
                                        .clone()
                                        .unwrap_or_default(),
                                    description: dev
                                        .Description
                                        .clone()
                                        .unwrap_or_default(),
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
                            known_devices.devices.get_mut(&event.device_id)
                        {
                            entry.last_seen = now_iso.clone();
                            entry.currently_connected = false;
                        }
                        storage_info.remove(&event.device_id);
                    }
                }
            }

            all_events.extend(new_events);
            save_cache(&known_devices);

            for id in enrich_ids {
                pending_enrichments.push((id, Instant::now()));
            }
        }

        if !all_events.is_empty()
            || enriched
            || prev.len() != current.len()
        {
            let mut sorted: Vec<_> =
                current.iter().map(|(id, d)| (id.clone(), d.clone())).collect();
            sorted.sort_by(|a, b| {
                a.1.display_name()
                    .to_lowercase()
                    .cmp(&b.1.display_name().to_lowercase())
            });
            let _ = tx.send(MonitorSnapshot {
                devices: sorted,
                events: all_events.clone(),
                known_devices: known_devices.clone(),
                storage_info: storage_info.clone(),
                error: None,
            });
        }

        prev = current;
    }
}

// ── Theme system ───────────────────────────────────────────────

#[derive(Clone, Copy, PartialEq, Debug)]
enum AppTheme {
    Neon,
    CatppuccinMocha,
    Dracula,
    Nord,
    Solarized,
}

impl AppTheme {
    const ALL: [AppTheme; 5] = [
        AppTheme::Neon,
        AppTheme::CatppuccinMocha,
        AppTheme::Dracula,
        AppTheme::Nord,
        AppTheme::Solarized,
    ];

    fn label(self) -> &'static str {
        match self {
            AppTheme::Neon => "Neon",
            AppTheme::CatppuccinMocha => "Mocha",
            AppTheme::Dracula => "Dracula",
            AppTheme::Nord => "Nord",
            AppTheme::Solarized => "Solar",
        }
    }

    fn from_label(s: &str) -> Self {
        match s {
            "Mocha" | "CatppuccinMocha" => AppTheme::CatppuccinMocha,
            "Dracula" => AppTheme::Dracula,
            "Nord" => AppTheme::Nord,
            "Solar" | "Solarized" => AppTheme::Solarized,
            _ => AppTheme::Neon,
        }
    }

    fn save_key(self) -> &'static str {
        match self {
            AppTheme::Neon => "Neon",
            AppTheme::CatppuccinMocha => "CatppuccinMocha",
            AppTheme::Dracula => "Dracula",
            AppTheme::Nord => "Nord",
            AppTheme::Solarized => "Solar",
        }
    }

    fn iced_theme(self) -> Theme {
        let tc = self.colors();
        Theme::custom(
            self.label().to_string(),
            iced::theme::Palette {
                background: tc.bg_deep,
                text: tc.text,
                primary: tc.accent,
                success: tc.green,
                danger: tc.red,
            },
        )
    }

    fn colors(self) -> ThemeColors {
        match self {
            AppTheme::Neon => ThemeColors {
                bg_deep: color!(0x0d, 0x0f, 0x14),
                bg_surface: color!(0x1a, 0x1c, 0x23),
                bg_elevated: color!(0x22, 0x25, 0x2e),
                border: color!(0x2a, 0x2d, 0x3a),
                accent: color!(0xa8, 0x55, 0xf7),
                orange: color!(0xff, 0x8b, 0x3d),
                teal: color!(0x2e, 0xe6, 0xd7),
                green: color!(0x50, 0xfa, 0x7b),
                red: color!(0xff, 0x55, 0x55),
                yellow: color!(0xf1, 0xfa, 0x8c),
                pink: color!(0xff, 0x79, 0xc6),
                cyan: color!(0x8b, 0xe9, 0xfd),
                text: color!(0xe8, 0xe8, 0xf0),
                text_sec: color!(0x8c, 0x8c, 0xa0),
                text_muted: color!(0x72, 0x74, 0x88),
            },
            AppTheme::CatppuccinMocha => ThemeColors {
                bg_deep: color!(0x1e, 0x1e, 0x2e),
                bg_surface: color!(0x28, 0x28, 0x3a),
                bg_elevated: color!(0x36, 0x37, 0x4a),
                border: color!(0x58, 0x5b, 0x70),
                accent: color!(0xca, 0xa6, 0xf7),
                orange: color!(0xfa, 0xb3, 0x87),
                teal: color!(0x94, 0xe2, 0xd5),
                green: color!(0xa6, 0xe3, 0xa1),
                red: color!(0xf3, 0x8b, 0xa8),
                yellow: color!(0xf9, 0xe2, 0xaf),
                pink: color!(0xf5, 0xc2, 0xe7),
                cyan: color!(0x89, 0xdc, 0xeb),
                text: color!(0xcd, 0xd6, 0xf4),
                text_sec: color!(0xba, 0xc2, 0xde),
                text_muted: color!(0x7f, 0x84, 0x9c),
            },
            AppTheme::Dracula => ThemeColors {
                bg_deep: color!(0x28, 0x2a, 0x36),
                bg_surface: color!(0x30, 0x32, 0x40),
                bg_elevated: color!(0x3c, 0x3f, 0x50),
                border: color!(0x62, 0x72, 0xa4),
                accent: color!(0xbd, 0x93, 0xf9),
                orange: color!(0xff, 0xb8, 0x6c),
                teal: color!(0x8b, 0xe9, 0xfd),
                green: color!(0x50, 0xfa, 0x7b),
                red: color!(0xff, 0x55, 0x55),
                yellow: color!(0xf1, 0xfa, 0x8c),
                pink: color!(0xff, 0x79, 0xc6),
                cyan: color!(0x8b, 0xe9, 0xfd),
                text: color!(0xf8, 0xf8, 0xf2),
                text_sec: color!(0xd0, 0xd2, 0xdc),
                text_muted: color!(0x7e, 0x8c, 0xb4),
            },
            AppTheme::Nord => ThemeColors {
                bg_deep: color!(0x2e, 0x34, 0x40),
                bg_surface: color!(0x35, 0x3b, 0x49),
                bg_elevated: color!(0x3e, 0x45, 0x55),
                border: color!(0x4c, 0x56, 0x6a),
                accent: color!(0x88, 0xc0, 0xd0),
                orange: color!(0xd0, 0x87, 0x70),
                teal: color!(0x8f, 0xbc, 0xbb),
                green: color!(0xa3, 0xbe, 0x8c),
                red: color!(0xbf, 0x61, 0x6a),
                yellow: color!(0xeb, 0xcb, 0x8b),
                pink: color!(0xb4, 0x8e, 0xad),
                cyan: color!(0x88, 0xc0, 0xd0),
                text: color!(0xec, 0xef, 0xf4),
                text_sec: color!(0xd8, 0xde, 0xe9),
                text_muted: color!(0x7b, 0x88, 0xa0),
            },
            AppTheme::Solarized => ThemeColors {
                bg_deep: color!(0x00, 0x2b, 0x36),
                bg_surface: color!(0x07, 0x36, 0x42),
                bg_elevated: color!(0x0e, 0x40, 0x4d),
                border: color!(0x58, 0x6e, 0x75),
                accent: color!(0x26, 0x8b, 0xd2),
                orange: color!(0xcb, 0x4b, 0x16),
                teal: color!(0x2a, 0xa1, 0x98),
                green: color!(0x85, 0x99, 0x00),
                red: color!(0xdc, 0x32, 0x2f),
                yellow: color!(0xb5, 0x89, 0x00),
                pink: color!(0xd3, 0x36, 0x82),
                cyan: color!(0x2a, 0xa1, 0x98),
                text: color!(0xfd, 0xf6, 0xe3),
                text_sec: color!(0x93, 0xa1, 0xa1),
                text_muted: color!(0x65, 0x7b, 0x83),
            },
        }
    }
}

#[derive(Clone, Copy)]
struct ThemeColors {
    bg_deep: iced::Color,
    bg_surface: iced::Color,
    bg_elevated: iced::Color,
    border: iced::Color,
    accent: iced::Color,
    orange: iced::Color,
    teal: iced::Color,
    green: iced::Color,
    red: iced::Color,
    yellow: iced::Color,
    pink: iced::Color,
    cyan: iced::Color,
    text: iced::Color,
    text_sec: iced::Color,
    text_muted: iced::Color,
}

// ── Tab + Sort enums ───────────────────────────────────────────

#[derive(Clone, Copy, PartialEq, Debug)]
enum ActiveTab {
    Monitor,
    KnownDevices,
}

impl ActiveTab {
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

#[derive(Clone, Copy, PartialEq, Debug)]
enum SortMode {
    Status,
    Name,
    LastSeen,
    TimesSeen,
    FirstSeen,
}

impl SortMode {
    const ALL: [SortMode; 5] = [
        SortMode::Status,
        SortMode::Name,
        SortMode::LastSeen,
        SortMode::TimesSeen,
        SortMode::FirstSeen,
    ];

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

// ── Tray ────────────────────────────────────────────────────────

#[derive(Clone, Debug)]
enum TrayAction {
    Show,
    Hide,
    Exit,
}

struct TrayMenuIds {
    show: tray_icon::menu::MenuId,
    hide: tray_icon::menu::MenuId,
    exit: tray_icon::menu::MenuId,
}

// ── Messages ───────────────────────────────────────────────────

#[derive(Clone, Debug)]
enum Message {
    // Background monitor
    PollMonitor,
    #[allow(dead_code)]
    MonitorUpdate(MonitorSnapshot),

    // UI
    TabSelected(ActiveTab),
    ThemeChanged(AppTheme),
    SearchChanged(String),
    SortBy(SortMode),
    #[allow(dead_code)]
    ToggleSortDirection,
    SelectDevice(Option<String>),
    NicknameChanged(String),
    SaveNickname,
    ForgetDevice(String),
    ClearEvents,
    CopyToClipboard(String),
    OpenUrl(String),

    // System
    UpdateAvailable(Option<String>),
    PollTray,
    #[allow(dead_code)]
    TrayEvent(TrayAction),
    #[allow(dead_code)]
    CloseRequested,

    // System (reserved for future close handling)
    #[allow(dead_code)]
    Noop,
}

impl std::fmt::Debug for MonitorSnapshot {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("MonitorSnapshot")
            .field("devices", &self.devices.len())
            .field("events", &self.events.len())
            .finish()
    }
}

// ── Iced Application ───────────────────────────────────────────

struct DeviceHistoryApp {
    // Data
    devices: Vec<(String, UsbDevice)>,
    events: Vec<DeviceEvent>,
    known_devices: KnownDeviceCache,
    storage_info: HashMap<String, StorageInfo>,
    error: Option<String>,

    // Monitor channel
    monitor_rx: Arc<Mutex<mpsc::Receiver<MonitorSnapshot>>>,

    // UI state
    app_theme: AppTheme,
    colors: ThemeColors,
    active_tab: ActiveTab,
    search_query: String,
    sort_mode: SortMode,
    sort_ascending: bool,
    selected_device: Option<String>,
    nickname_buf: String,

    // System
    update_available: Option<String>,
    #[allow(dead_code)]
    tray_menu_ids: Arc<TrayMenuIds>,
    tray_rx: Arc<Mutex<mpsc::Receiver<TrayAction>>>,
}

impl DeviceHistoryApp {
    fn new(
        monitor_rx: mpsc::Receiver<MonitorSnapshot>,
        tray_menu_ids: TrayMenuIds,
        tray_rx: mpsc::Receiver<TrayAction>,
    ) -> (Self, IcedTask<Message>) {
        let prefs = Prefs::load();
        let app_theme = AppTheme::from_label(&prefs.theme);

        let app = Self {
            devices: vec![],
            events: vec![],
            known_devices: load_cache(),
            storage_info: HashMap::new(),
            error: None,
            monitor_rx: Arc::new(Mutex::new(monitor_rx)),
            app_theme,
            colors: app_theme.colors(),
            active_tab: ActiveTab::from_label(&prefs.active_tab),
            search_query: String::new(),
            sort_mode: SortMode::Status,
            sort_ascending: true,
            selected_device: None,
            nickname_buf: String::new(),
            update_available: None,
            tray_menu_ids: Arc::new(tray_menu_ids),
            tray_rx: Arc::new(Mutex::new(tray_rx)),
        };

        // Kick off update check
        let task = IcedTask::perform(check_for_updates(), Message::UpdateAvailable);

        (app, task)
    }

    fn save_prefs(&self) {
        let prefs = Prefs {
            theme: self.app_theme.save_key().to_string(),
            active_tab: self.active_tab.save_key().to_string(),
        };
        prefs.save();
    }

    fn update(&mut self, message: Message) -> IcedTask<Message> {
        match message {
            Message::PollMonitor => {
                if let Ok(rx) = self.monitor_rx.lock() {
                    while let Ok(snap) = rx.try_recv() {
                        self.devices = snap.devices;
                        self.events = snap.events;
                        self.known_devices = snap.known_devices;
                        self.storage_info = snap.storage_info;
                        if snap.error.is_some() {
                            self.error = snap.error;
                        }
                    }
                }
            }
            Message::MonitorUpdate(snap) => {
                self.devices = snap.devices;
                self.events = snap.events;
                self.known_devices = snap.known_devices;
                self.storage_info = snap.storage_info;
                if snap.error.is_some() {
                    self.error = snap.error;
                }
            }
            Message::TabSelected(tab) => {
                self.active_tab = tab;
                self.selected_device = None;
                self.save_prefs();
            }
            Message::ThemeChanged(theme) => {
                self.app_theme = theme;
                self.colors = theme.colors();
                self.save_prefs();
            }
            Message::SearchChanged(q) => {
                self.search_query = q;
            }
            Message::SortBy(mode) => {
                if self.sort_mode == mode {
                    self.sort_ascending = !self.sort_ascending;
                } else {
                    self.sort_mode = mode;
                    self.sort_ascending = true;
                }
            }
            Message::ToggleSortDirection => {
                self.sort_ascending = !self.sort_ascending;
            }
            Message::SelectDevice(id) => {
                if let Some(ref dev_id) = id {
                    if let Some(kd) = self.known_devices.devices.get(dev_id) {
                        self.nickname_buf = kd.nickname.clone().unwrap_or_default();
                    } else {
                        self.nickname_buf.clear();
                    }
                }
                self.selected_device = id;
            }
            Message::NicknameChanged(s) => {
                self.nickname_buf = s;
            }
            Message::SaveNickname => {
                if let Some(ref dev_id) = self.selected_device {
                    let nick = if self.nickname_buf.trim().is_empty() {
                        None
                    } else {
                        Some(self.nickname_buf.trim().to_string())
                    };
                    if let Some(kd) = self.known_devices.devices.get_mut(dev_id) {
                        kd.nickname = nick;
                    }
                    save_cache(&self.known_devices);
                }
            }
            Message::ForgetDevice(id) => {
                self.known_devices.devices.remove(&id);
                self.storage_info.remove(&id);
                save_cache(&self.known_devices);
                if self.selected_device.as_deref() == Some(&id) {
                    self.selected_device = None;
                }
            }
            Message::ClearEvents => {
                self.events.clear();
            }
            Message::CopyToClipboard(s) => {
                return iced::clipboard::write(s);
            }
            Message::OpenUrl(url) => {
                let _ = open::that(&url);
            }
            Message::UpdateAvailable(ver) => {
                self.update_available = ver;
            }
            Message::PollTray => {
                if let Ok(rx) = self.tray_rx.lock() {
                    while let Ok(action) = rx.try_recv() {
                        match action {
                            TrayAction::Show => {
                                #[cfg(windows)]
                                win32::show_window();
                            }
                            TrayAction::Hide => {
                                #[cfg(windows)]
                                win32::hide_window();
                            }
                            TrayAction::Exit => {
                                std::process::exit(0);
                            }
                        }
                    }
                }
            }
            Message::TrayEvent(action) => match action {
                TrayAction::Show => {
                    #[cfg(windows)]
                    win32::show_window();
                }
                TrayAction::Hide => {
                    #[cfg(windows)]
                    win32::hide_window();
                }
                TrayAction::Exit => {
                    std::process::exit(0);
                }
            },
            Message::CloseRequested => {
                #[cfg(windows)]
                win32::hide_window();
            }
            Message::Noop => {}
        }
        IcedTask::none()
    }

    fn subscription(&self) -> Subscription<Message> {
        Subscription::batch([
            iced::time::every(Duration::from_millis(200)).map(|_| Message::PollMonitor),
            iced::time::every(Duration::from_millis(100)).map(|_| Message::PollTray),
        ])
    }

    fn theme(&self) -> Theme {
        self.app_theme.iced_theme()
    }

    fn title(&self) -> String {
        "Device History".to_string()
    }

    // ── View ───────────────────────────────────────────────────

    fn view(&self) -> Element<'_, Message> {
        let tc = &self.colors;

        let known_total = self.known_devices.devices.len();
        let known_online = self
            .known_devices
            .devices
            .values()
            .filter(|d| d.currently_connected)
            .count();

        // ── Header row ──
        let header = self.view_header(&tc);

        // ── Tab bar ──
        let tab_bar = self.view_tab_bar(&tc, known_total, known_online);

        // ── Content ──
        let content = match self.active_tab {
            ActiveTab::Monitor => self.view_monitor_tab(&tc),
            ActiveTab::KnownDevices => self.view_known_devices_tab(&tc),
        };

        // ── Footer ──
        let footer = self.view_footer(&tc, known_total, known_online);

        let layout = column![header, tab_bar, content, footer];

        container(layout)
            .width(Length::Fill)
            .height(Length::Fill)
            .style(move |_theme: &Theme| container::Style {
                background: Some(iced::Background::Color(tc.bg_deep)),
                ..Default::default()
            })
            .into()
    }

    fn view_header<'a>(&'a self, tc: &'a ThemeColors) -> Element<'a, Message> {
        let title = text("Device History")
            .size(20)
            .color(tc.accent);
        let version = text(format!("v{}", env!("CARGO_PKG_VERSION")))
            .size(11)
            .color(tc.text_muted);

        let mut header_row = Row::new()
            .spacing(6)
            .align_y(iced::Alignment::Center)
            .push(title)
            .push(version);

        // Update badge
        if let Some(ref ver) = self.update_available {
            let update_btn = button(
                text(format!("Update: v{}", ver))
                    .size(11)
                    .color(tc.orange),
            )
            .on_press(Message::OpenUrl(
                "https://github.com/TrentSterling/device-history/releases/latest".to_string(),
            ))
            .padding([3, 8])
            .style(move |_theme: &Theme, status| {
                let bg = match status {
                    button::Status::Hovered => blend_color(tc.bg_surface, tc.orange, 0.15),
                    button::Status::Pressed => blend_color(tc.bg_surface, tc.orange, 0.25),
                    _ => iced::Color::TRANSPARENT,
                };
                button::Style {
                    background: Some(iced::Background::Color(bg)),
                    border: iced::Border {
                        color: tc.orange,
                        width: 1.0,
                        radius: 4.0.into(),
                    },
                    text_color: tc.orange,
                    ..Default::default()
                }
            });
            header_row = header_row.push(update_btn);
        }

        header_row = header_row.push(horizontal_space());

        // Theme buttons
        for t in AppTheme::ALL {
            let selected = self.app_theme == t;
            let label_color = if selected { tc.accent } else { tc.text_sec };
            let base_bg = if selected {
                blend_color(tc.bg_elevated, tc.accent, 0.12)
            } else {
                iced::Color::TRANSPARENT
            };
            let border_color = if selected { tc.accent } else { tc.border };

            let theme_btn = button(text(t.label()).size(11).color(label_color))
                .on_press(Message::ThemeChanged(t))
                .padding([3, 8])
                .style(move |_theme: &Theme, status| {
                    let bg = match status {
                        button::Status::Hovered => blend_color(
                            if selected { base_bg } else { tc.bg_elevated },
                            tc.accent,
                            0.18,
                        ),
                        button::Status::Pressed => blend_color(
                            if selected { base_bg } else { tc.bg_elevated },
                            tc.accent,
                            0.28,
                        ),
                        _ => base_bg,
                    };
                    button::Style {
                        background: Some(iced::Background::Color(bg)),
                        border: iced::Border {
                            color: border_color,
                            width: if selected { 1.0 } else { 0.5 },
                            radius: 3.0.into(),
                        },
                        text_color: label_color,
                        ..Default::default()
                    }
                });
            header_row = header_row.push(theme_btn);
        }

        // Status
        header_row = header_row.push(Space::with_width(12));
        if let Some(ref err) = self.error {
            header_row = header_row.push(text(err).size(13).color(tc.pink));
        } else {
            header_row = header_row.push(
                text(format!("\u{25CF} Monitoring {} devices", self.devices.len()))
                    .size(13)
                    .color(tc.teal),
            );
        }

        let bg = tc.bg_surface;
        let accent_hint = blend_color(tc.bg_surface, tc.accent, 0.05);
        let border = tc.border;
        container(header_row.width(Length::Fill))
            .padding([10, 14])
            .style(move |_theme: &Theme| container::Style {
                background: Some(iced::Background::Gradient(iced::Gradient::Linear(
                    iced::gradient::Linear::new(std::f32::consts::FRAC_PI_2)
                        .add_stop(0.0, bg)
                        .add_stop(1.0, accent_hint),
                ))),
                border: iced::Border {
                    color: border,
                    width: 1.0,
                    radius: 0.0.into(),
                },
                ..Default::default()
            })
            .into()
    }

    fn view_tab_bar<'a>(
        &'a self,
        tc: &'a ThemeColors,
        known_total: usize,
        known_online: usize,
    ) -> Element<'a, Message> {
        let mut tab_row = Row::new().spacing(4).align_y(iced::Alignment::Center);

        let tab_labels = [
            (ActiveTab::Monitor, "Monitor"),
            (ActiveTab::KnownDevices, "Known Devices"),
        ];

        for (tab, label) in tab_labels {
            let selected = self.active_tab == tab;
            let text_color = if selected { tc.accent } else { tc.text_sec };
            let base_fill = if selected { tc.bg_deep } else { tc.bg_elevated };
            let border_color = if selected { tc.accent } else { tc.border };

            let tab_btn = button(text(label).size(13).color(text_color))
                .on_press(Message::TabSelected(tab))
                .padding([6, 14])
                .style(move |_theme: &Theme, status| {
                    let fill = match status {
                        button::Status::Hovered => blend_color(base_fill, tc.accent, 0.12),
                        button::Status::Pressed => blend_color(base_fill, tc.accent, 0.20),
                        _ => base_fill,
                    };
                    button::Style {
                        background: Some(iced::Background::Color(fill)),
                        border: iced::Border {
                            color: border_color,
                            width: if selected { 1.0 } else { 0.5 },
                            radius: 6.0.into(),
                        },
                        text_color,
                        ..Default::default()
                    }
                });
            tab_row = tab_row.push(tab_btn);
        }

        tab_row = tab_row.push(horizontal_space());
        tab_row = tab_row.push(
            text(format!("{} known ({} online)", known_total, known_online))
                .size(12)
                .color(tc.text_muted),
        );

        let bg = tc.bg_surface;
        let border = tc.border;
        container(tab_row.width(Length::Fill))
            .padding(iced::Padding { top: 6.0, right: 14.0, bottom: 0.0, left: 14.0 })
            .style(move |_theme: &Theme| container::Style {
                background: Some(iced::Background::Color(bg)),
                border: iced::Border {
                    color: border,
                    width: 0.5,
                    radius: 0.0.into(),
                },
                ..Default::default()
            })
            .into()
    }

    fn view_footer<'a>(
        &'a self,
        tc: &'a ThemeColors,
        known_total: usize,
        known_online: usize,
    ) -> Element<'a, Message> {
        let footer_row = row![
            text(format!("{} events logged", self.events.len()))
                .size(12)
                .color(tc.text_muted),
            text("|").size(10).color(tc.accent),
            text(format!("{} known ({} online)", known_total, known_online))
                .size(12)
                .color(tc.text_muted),
            text("|").size(10).color(tc.accent),
            text("Log: device-history.log")
                .size(12)
                .color(tc.text_muted),
            horizontal_space(),
            text("tront.xyz").size(11).color(tc.text_muted),
        ]
        .spacing(6)
        .align_y(iced::Alignment::Center);

        let bg = tc.bg_surface;
        let border = tc.border;
        container(footer_row.width(Length::Fill))
            .padding([7, 14])
            .style(move |_theme: &Theme| container::Style {
                background: Some(iced::Background::Color(bg)),
                border: iced::Border {
                    color: border,
                    width: 1.0,
                    radius: 0.0.into(),
                },
                ..Default::default()
            })
            .into()
    }

    // ── Monitor Tab ────────────────────────────────────────────

    fn view_monitor_tab<'a>(&'a self, tc: &'a ThemeColors) -> Element<'a, Message> {
        let mut content = Column::new().spacing(8).padding(14);

        // Compact about banner
        content = content.push(self.view_about_banner(tc));

        // Event log header
        let clear_btn = button(text("\u{00D7} Clear").size(12).color(tc.orange))
            .on_press(Message::ClearEvents)
            .padding([2, 8])
            .style(move |_theme: &Theme, status| {
                let bg = match status {
                    button::Status::Hovered => blend_color(tc.bg_surface, tc.orange, 0.15),
                    button::Status::Pressed => blend_color(tc.bg_surface, tc.orange, 0.25),
                    _ => iced::Color::TRANSPARENT,
                };
                button::Style {
                    background: Some(iced::Background::Color(bg)),
                    border: iced::Border {
                        color: tc.orange,
                        width: 1.0,
                        radius: 4.0.into(),
                    },
                    text_color: tc.orange,
                    ..Default::default()
                }
            });
        let event_header = row![
            text(format!(">Event Log ({})", self.events.len()))
                .size(15)
                .color(tc.cyan),
            horizontal_space(),
            clear_btn,
        ]
        .align_y(iced::Alignment::Center);
        content = content.push(event_header);

        // Event list
        content = content.push(self.view_event_list(tc));

        // Rainbow separator
        content = content.push(rainbow_separator(tc));

        // Connected devices header
        content = content.push(
            text(format!(">Connected ({})", self.devices.len()))
                .size(15)
                .color(tc.cyan),
        );

        // Connected devices list
        content = content.push(self.view_connected_devices(tc));

        let bg = tc.bg_deep;
        container(content.width(Length::Fill).height(Length::Fill))
            .width(Length::Fill)
            .height(Length::Fill)
            .style(move |_theme: &Theme| container::Style {
                background: Some(iced::Background::Color(bg)),
                ..Default::default()
            })
            .into()
    }

    fn view_about_banner<'a>(&'a self, tc: &'a ThemeColors) -> Element<'a, Message> {
        let banner_row = row![
            text(">WTF just disconnected?").size(13).color(tc.accent),
            text("\u{2014}").size(11).color(tc.text_muted),
            text("Real-time USB monitor").size(11).color(tc.text_sec),
            text("|").size(10).color(tc.border),
            text("\u{25CF} Live").size(10).color(tc.green),
            text("\u{25CF} Log").size(10).color(tc.cyan),
            text("\u{25CF} Cache").size(10).color(tc.yellow),
            text("\u{25CF} CLI").size(10).color(tc.orange),
        ]
        .spacing(8)
        .align_y(iced::Alignment::Center);

        let bg_left = tc.bg_surface;
        let bg_right = blend_color(tc.bg_surface, tc.accent, 0.06);
        let border = tc.border;
        container(banner_row)
            .padding([6, 12])
            .width(Length::Fill)
            .style(move |_theme: &Theme| container::Style {
                background: Some(iced::Background::Gradient(iced::Gradient::Linear(
                    iced::gradient::Linear::new(std::f32::consts::FRAC_PI_2)
                        .add_stop(0.0, bg_left)
                        .add_stop(1.0, bg_right),
                ))),
                border: iced::Border {
                    color: border,
                    width: 0.5,
                    radius: 6.0.into(),
                },
                ..Default::default()
            })
            .into()
    }

    fn view_event_list<'a>(&'a self, tc: &'a ThemeColors) -> Element<'a, Message> {
        let mut event_col = Column::new().spacing(3);

        if self.events.is_empty() {
            event_col = event_col.push(
                container(
                    text("No events yet -- waiting for USB changes...")
                        .size(13)
                        .color(tc.text_sec),
                )
                .padding(16)
                .width(Length::Fill)
                .center_x(Length::Fill),
            );
        } else {
            for event in &self.events {
                event_col = event_col.push(self.view_event_card(tc, event));
            }
        }

        let bg = tc.bg_surface;
        let border = tc.border;
        container(scrollable(event_col).height(Length::Fill))
            .padding(iced::Padding { top: 6.0, right: 14.0, bottom: 6.0, left: 6.0 })
            .width(Length::Fill)
            .style(move |_theme: &Theme| container::Style {
                background: Some(iced::Background::Color(bg)),
                border: iced::Border {
                    color: border,
                    width: 1.0,
                    radius: 6.0.into(),
                },
                ..Default::default()
            })
            .into()
    }

    fn view_event_card<'a>(&'a self, tc: &'a ThemeColors, event: &'a DeviceEvent) -> Element<'a, Message> {
        let is_selected = self.selected_device.as_deref() == Some(&event.device_id);
        let (accent, icon, label) = match event.kind {
            EventKind::Connect => (tc.green, "\u{25B2}", "CONNECT"),
            EventKind::Disconnect => (tc.red, "\u{25BC}", "DISCONNECT"),
        };

        let card_bg = if is_selected {
            blend_color(tc.bg_elevated, tc.accent, 0.12)
        } else {
            blend_color(tc.bg_elevated, accent, 0.05)
        };
        let card_border = if is_selected { tc.accent } else { tc.border };

        let mut card_row = Row::new()
            .spacing(6)
            .align_y(iced::Alignment::Center)
            .push(text(&event.timestamp).size(12).color(tc.text_sec))
            .push(
                text(format!("{} {}", icon, label))
                    .size(12)
                    .color(accent),
            )
            .push(text(&event.name).size(12).color(tc.text));

        if let Some(ref vp) = event.vid_pid {
            card_row = card_row.push(text(format!("[{}]", vp)).size(11).color(tc.yellow));
        }

        // Drive letter badge
        if let Some(si) = self.storage_info.get(&event.device_id) {
            for vol in &si.volumes {
                card_row = card_row.push(
                    text(format!("[{}]", vol.drive_letter))
                        .size(11)
                        .color(tc.green),
                );
            }
        }

        card_row = card_row.push(text(&event.class).size(10).color(tc.accent));

        if let Some(ref mfr) = event.manufacturer {
            card_row = card_row.push(text(format!("({})", mfr)).size(11).color(tc.text_sec));
        }

        let dev_id = event.device_id.clone();
        let card = button(
            container(card_row)
                .padding([4, 10])
                .width(Length::Fill),
        )
        .on_press(if is_selected {
            Message::SelectDevice(None)
        } else {
            Message::SelectDevice(Some(dev_id.clone()))
        })
        .padding(0)
        .style(move |_theme: &Theme, status| {
            let bg = match status {
                button::Status::Hovered => blend_color(card_bg, tc.accent, 0.08),
                button::Status::Pressed => blend_color(card_bg, tc.accent, 0.15),
                _ => card_bg,
            };
            let bw = match status {
                button::Status::Hovered => 1.0,
                _ => if is_selected { 1.5 } else { 0.5 },
            };
            button::Style {
                background: Some(iced::Background::Color(bg)),
                border: iced::Border {
                    color: card_border,
                    width: bw,
                    radius: 4.0.into(),
                },
                ..Default::default()
            }
        });

        let mut col = Column::new().push(card);
        if is_selected {
            col = col.push(self.view_detail_panel(tc, &event.device_id, true));
        }
        col.into()
    }

    fn view_connected_devices<'a>(&'a self, tc: &'a ThemeColors) -> Element<'a, Message> {
        let mut dev_col = Column::new().spacing(2);

        if self.devices.is_empty() {
            dev_col = dev_col.push(
                container(
                    text("No devices connected")
                        .size(13)
                        .color(tc.text_sec),
                )
                .padding(16)
                .width(Length::Fill)
                .center_x(Length::Fill),
            );
        } else {
            for (dev_id, dev) in &self.devices {
                dev_col = dev_col.push(self.view_connected_device_card(tc, dev_id, dev));
            }
        }

        let bg = tc.bg_surface;
        let border = tc.border;
        container(scrollable(dev_col).height(Length::Fill))
            .padding(iced::Padding { top: 6.0, right: 14.0, bottom: 6.0, left: 6.0 })
            .width(Length::Fill)
            .style(move |_theme: &Theme| container::Style {
                background: Some(iced::Background::Color(bg)),
                border: iced::Border {
                    color: border,
                    width: 1.0,
                    radius: 6.0.into(),
                },
                ..Default::default()
            })
            .into()
    }

    fn view_connected_device_card<'a>(
        &'a self,
        tc: &'a ThemeColors,
        dev_id: &'a str,
        dev: &'a UsbDevice,
    ) -> Element<'a, Message> {
        let is_selected = self.selected_device.as_deref() == Some(dev_id);
        let card_bg = if is_selected {
            blend_color(tc.bg_elevated, tc.accent, 0.12)
        } else {
            tc.bg_elevated
        };
        let card_border = if is_selected { tc.accent } else { tc.border };

        let conn_si = self.storage_info.get(dev_id);
        let mut card_row = Row::new()
            .spacing(6)
            .align_y(iced::Alignment::Center);

        if let Some(si) = conn_si {
            for vol in &si.volumes {
                card_row = card_row.push(
                    text(&vol.drive_letter)
                        .size(13)
                        .color(tc.green),
                );
                if !vol.volume_name.is_empty() {
                    card_row = card_row.push(
                        text(format!("\"{}\"", vol.volume_name))
                            .size(12)
                            .color(tc.text),
                    );
                }
            }
            if !si.model.is_empty() {
                card_row = card_row.push(text(&si.model).size(11).color(tc.text_sec));
            }
        } else {
            card_row = card_row.push(text(dev.class()).size(11).color(tc.accent));
            card_row = card_row.push(text(dev.display_name()).size(12).color(tc.text));
        }

        // Nickname
        if let Some(kd) = self.known_devices.devices.get(dev_id) {
            if let Some(ref nick) = kd.nickname {
                if !nick.is_empty() {
                    card_row =
                        card_row.push(text(format!("({})", nick)).size(11).color(tc.teal));
                }
            }
        }

        if let Some(vp) = dev.vid_pid() {
            card_row = card_row.push(text(format!("[{}]", vp)).size(10).color(tc.yellow));
        }

        if conn_si.is_none() {
            if let Some(ref mfr) = dev.Manufacturer {
                card_row = card_row.push(text(mfr).size(10).color(tc.text_sec));
            }
        }

        let dev_id_string = dev_id.to_string();
        let card = button(
            container(card_row)
                .padding([4, 10])
                .width(Length::Fill),
        )
        .on_press(if is_selected {
            Message::SelectDevice(None)
        } else {
            Message::SelectDevice(Some(dev_id_string))
        })
        .padding(0)
        .style(move |_theme: &Theme, status| {
            let bg = match status {
                button::Status::Hovered => blend_color(card_bg, tc.accent, 0.08),
                button::Status::Pressed => blend_color(card_bg, tc.accent, 0.15),
                _ => card_bg,
            };
            let bw = match status {
                button::Status::Hovered => 1.0,
                _ => if is_selected { 1.5 } else { 0.5 },
            };
            button::Style {
                background: Some(iced::Background::Color(bg)),
                border: iced::Border {
                    color: card_border,
                    width: bw,
                    radius: 4.0.into(),
                },
                ..Default::default()
            }
        });

        let mut col = Column::new().push(card);
        if is_selected {
            col = col.push(self.view_detail_panel(tc, dev_id, true));
        }
        col.into()
    }

    // ── Known Devices Tab ──────────────────────────────────────

    fn view_known_devices_tab<'a>(&'a self, tc: &'a ThemeColors) -> Element<'a, Message> {
        let mut content = Column::new().spacing(6).padding(14);

        // Search + sort bar
        let search_input = text_input("Search by name, class, manufacturer, VID:PID...", &self.search_query)
            .on_input(Message::SearchChanged)
            .width(300)
            .size(13);

        let mut sort_row = Row::new().spacing(4).align_y(iced::Alignment::Center);
        sort_row = sort_row
            .push(text(">").size(14).color(tc.text_sec))
            .push(search_input);

        // Search clear (X) button
        if !self.search_query.is_empty() {
            let clear_btn = button(text("\u{2715}").size(14).color(tc.text_sec))
                .on_press(Message::SearchChanged(String::new()))
                .padding([2, 6])
                .style(move |_theme: &Theme, status| {
                    let bg = match status {
                        button::Status::Hovered => blend_color(tc.bg_surface, tc.red, 0.15),
                        button::Status::Pressed => blend_color(tc.bg_surface, tc.red, 0.25),
                        _ => iced::Color::TRANSPARENT,
                    };
                    button::Style {
                        background: Some(iced::Background::Color(bg)),
                        border: iced::Border {
                            color: iced::Color::TRANSPARENT,
                            width: 0.0,
                            radius: 3.0.into(),
                        },
                        text_color: tc.text_sec,
                        ..Default::default()
                    }
                });
            sort_row = sort_row.push(clear_btn);
        }

        sort_row = sort_row.push(Space::with_width(12));

        for mode in SortMode::ALL {
            let selected = self.sort_mode == mode;
            let arrow = if selected {
                if self.sort_ascending { " \u{25B2}" } else { " \u{25BC}" }
            } else {
                ""
            };
            let label_color = if selected { tc.accent } else { tc.text_sec };
            let base_bg = if selected {
                blend_color(tc.bg_elevated, tc.accent, 0.12)
            } else {
                iced::Color::TRANSPARENT
            };
            let border_color = if selected { tc.accent } else { tc.border };

            let sort_btn = button(
                text(format!("{}{}", mode.label(), arrow))
                    .size(11)
                    .color(label_color),
            )
            .on_press(Message::SortBy(mode))
            .padding([3, 8])
            .style(move |_theme: &Theme, status| {
                let bg = match status {
                    button::Status::Hovered => blend_color(
                        if selected { base_bg } else { tc.bg_elevated },
                        tc.accent,
                        0.18,
                    ),
                    button::Status::Pressed => blend_color(
                        if selected { base_bg } else { tc.bg_elevated },
                        tc.accent,
                        0.28,
                    ),
                    _ => base_bg,
                };
                button::Style {
                    background: Some(iced::Background::Color(bg)),
                    border: iced::Border {
                        color: border_color,
                        width: if selected { 1.0 } else { 0.5 },
                        radius: 3.0.into(),
                    },
                    text_color: label_color,
                    ..Default::default()
                }
            });
            sort_row = sort_row.push(sort_btn);
        }

        content = content.push(sort_row);

        // Filter + sort devices
        let query_lower = self.search_query.to_lowercase();
        let mut filtered: Vec<&KnownDevice> = self
            .known_devices
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
                    || d.nickname
                        .as_deref()
                        .unwrap_or("")
                        .to_lowercase()
                        .contains(&query_lower)
            })
            .collect();

        let sort_mode = self.sort_mode;
        let sort_ascending = self.sort_ascending;
        filtered.sort_by(|a, b| {
            let cmp = match sort_mode {
                SortMode::Status => a
                    .currently_connected
                    .cmp(&b.currently_connected)
                    .then_with(|| a.name.to_lowercase().cmp(&b.name.to_lowercase())),
                SortMode::Name => a.name.to_lowercase().cmp(&b.name.to_lowercase()),
                SortMode::LastSeen => a
                    .last_seen
                    .cmp(&b.last_seen)
                    .then_with(|| a.name.to_lowercase().cmp(&b.name.to_lowercase())),
                SortMode::TimesSeen => a
                    .times_seen
                    .cmp(&b.times_seen)
                    .then_with(|| a.name.to_lowercase().cmp(&b.name.to_lowercase())),
                SortMode::FirstSeen => a
                    .first_seen
                    .cmp(&b.first_seen)
                    .then_with(|| a.name.to_lowercase().cmp(&b.name.to_lowercase())),
            };
            if sort_ascending {
                cmp
            } else {
                cmp.reverse()
            }
        });

        // Device cards
        let mut dev_col = Column::new().spacing(3);

        if self.known_devices.devices.is_empty() {
            dev_col = dev_col.push(
                container(
                    text("No devices seen yet -- plug in a USB device to get started")
                        .size(13)
                        .color(tc.text_sec),
                )
                .padding(24)
                .width(Length::Fill)
                .center_x(Length::Fill),
            );
        } else if filtered.is_empty() {
            dev_col = dev_col.push(
                container(
                    text(format!("No devices matching '{}'", self.search_query))
                        .size(13)
                        .color(tc.text_sec),
                )
                .padding(24)
                .width(Length::Fill)
                .center_x(Length::Fill),
            );
        } else {
            for dev in &filtered {
                dev_col = dev_col.push(self.view_known_device_card(tc, dev));
            }
        }

        let bg = tc.bg_surface;
        let border = tc.border;
        let devices_container = container(scrollable(dev_col).height(Length::Fill))
            .padding(iced::Padding { top: 6.0, right: 14.0, bottom: 6.0, left: 6.0 })
            .width(Length::Fill)
            .style(move |_theme: &Theme| container::Style {
                background: Some(iced::Background::Color(bg)),
                border: iced::Border {
                    color: border,
                    width: 1.0,
                    radius: 6.0.into(),
                },
                ..Default::default()
            });

        content = content.push(devices_container);

        let bg = tc.bg_deep;
        container(content)
            .width(Length::Fill)
            .height(Length::Fill)
            .style(move |_theme: &Theme| container::Style {
                background: Some(iced::Background::Color(bg)),
                ..Default::default()
            })
            .into()
    }

    fn view_known_device_card<'a>(&'a self, tc: &'a ThemeColors, dev: &'a KnownDevice) -> Element<'a, Message> {
        let is_selected = self.selected_device.as_deref() == Some(&dev.device_id);

        let card_bg = if is_selected {
            blend_color(tc.bg_elevated, tc.accent, 0.10)
        } else if dev.currently_connected {
            blend_color(tc.bg_elevated, tc.green, 0.06)
        } else {
            tc.bg_elevated
        };
        let card_border = if is_selected { tc.accent } else { tc.border };

        let dot_color = if dev.currently_connected {
            tc.green
        } else {
            tc.text_muted
        };

        let dev_si = self
            .storage_info
            .get(&dev.device_id)
            .or(dev.storage_info.as_ref());

        // Row 1
        let mut row1 = Row::new()
            .spacing(6)
            .align_y(iced::Alignment::Center)
            .push(text("\u{25CF}").size(10).color(dot_color));

        if let Some(si) = dev_si {
            for vol in &si.volumes {
                row1 = row1.push(text(&vol.drive_letter).size(13).color(tc.green));
                if !vol.volume_name.is_empty() {
                    row1 = row1.push(
                        text(format!("\"{}\"", vol.volume_name))
                            .size(12)
                            .color(tc.text),
                    );
                }
            }
            if !si.model.is_empty() {
                row1 = row1.push(text(&si.model).size(11).color(tc.text_sec));
            }
        } else {
            row1 = row1.push(text(&dev.class).size(11).color(tc.accent));
            row1 = row1.push(text(&dev.name).size(12).color(tc.text));
        }

        if let Some(ref nick) = dev.nickname {
            if !nick.is_empty() {
                row1 = row1.push(text(format!("({})", nick)).size(11).color(tc.teal));
            }
        }
        if !dev.vid_pid.is_empty() {
            row1 = row1.push(text(format!("[{}]", dev.vid_pid)).size(10).color(tc.yellow));
        }
        if dev_si.is_none() && !dev.manufacturer.is_empty() {
            row1 = row1.push(text(&dev.manufacturer).size(10).color(tc.text_sec));
        }

        // Row 2: timestamps
        let row2 = row![
            text(format!("First: {}", dev.first_seen))
                .size(10)
                .color(tc.text_muted),
            text(format!("Last: {}", dev.last_seen))
                .size(10)
                .color(tc.text_muted),
            text(format!("{}x", dev.times_seen))
                .size(10)
                .color(tc.teal),
        ]
        .spacing(12);

        let card_content = column![row1, row2].spacing(2);

        let card = button(
            container(card_content)
                .padding([5, 10])
                .width(Length::Fill),
        )
        .on_press(if is_selected {
            Message::SelectDevice(None)
        } else {
            Message::SelectDevice(Some(dev.device_id.clone()))
        })
        .padding(0)
        .style(move |_theme: &Theme, status| {
            let bg = match status {
                button::Status::Hovered => blend_color(card_bg, tc.accent, 0.08),
                button::Status::Pressed => blend_color(card_bg, tc.accent, 0.15),
                _ => card_bg,
            };
            let bw = match status {
                button::Status::Hovered => 1.0,
                _ => if is_selected { 1.5 } else { 0.5 },
            };
            button::Style {
                background: Some(iced::Background::Color(bg)),
                border: iced::Border {
                    color: card_border,
                    width: bw,
                    radius: 4.0.into(),
                },
                ..Default::default()
            }
        });

        let mut col = Column::new().push(card);
        if is_selected {
            col = col.push(self.view_detail_panel(tc, &dev.device_id, dev.currently_connected));
        }
        col.into()
    }

    // ── Detail Panel ───────────────────────────────────────────

    fn view_detail_panel<'a>(
        &'a self,
        tc: &'a ThemeColors,
        device_id: &'a str,
        is_connected: bool,
    ) -> Element<'a, Message> {
        let kd = self.known_devices.devices.get(device_id);
        let si = self
            .storage_info
            .get(device_id)
            .or_else(|| kd.and_then(|k| k.storage_info.as_ref()));

        let mut content = Column::new().spacing(4);

        // Storage headline
        if let Some(si) = si {
            for vol in &si.volumes {
                let mut vol_row = Row::new()
                    .spacing(8)
                    .align_y(iced::Alignment::Center)
                    .push(text(&vol.drive_letter).size(20).color(tc.green));
                if !vol.volume_name.is_empty() {
                    vol_row = vol_row.push(
                        text(format!("\"{}\"", vol.volume_name))
                            .size(16)
                            .color(tc.text),
                    );
                }
                vol_row = vol_row.push(
                    text(format!("({})", vol.file_system))
                        .size(12)
                        .color(tc.text_sec),
                );
                content = content.push(vol_row);

                // Capacity bar (progress_bar + text)
                if vol.total_bytes > 0 {
                    let free_str = format_bytes(vol.free_bytes);
                    let total_str = format_bytes(vol.total_bytes);
                    let used_pct =
                        (1.0 - (vol.free_bytes as f64 / vol.total_bytes as f64)) * 100.0;
                    let bar_color = if used_pct < 70.0 {
                        tc.green
                    } else if used_pct < 90.0 {
                        tc.yellow
                    } else {
                        tc.red
                    };
                    let bar_bg = tc.bg_elevated;
                    content = content.push(
                        progress_bar(0.0..=100.0, used_pct as f32)
                            .height(12)
                            .style(move |_theme: &Theme| {
                                iced::widget::progress_bar::Style {
                                    background: iced::Background::Color(bar_bg),
                                    bar: iced::Background::Color(bar_color),
                                    border: iced::Border {
                                        color: iced::Color::TRANSPARENT,
                                        width: 0.0,
                                        radius: 3.0.into(),
                                    },
                                }
                            }),
                    );
                    content = content.push(
                        text(format!(
                            "{} free / {}  ({:.0}% used)",
                            free_str, total_str, used_pct
                        ))
                        .size(11)
                        .color(bar_color),
                    );
                }
            }

            // Model + serial
            let mut model_row = Row::new().spacing(6);
            model_row = model_row.push(text(&si.model).size(12).color(tc.text));
            if !si.serial_number.is_empty() {
                model_row = model_row.push(
                    text(format!("({})", si.serial_number))
                        .size(11)
                        .color(tc.text_sec),
                );
            }
            content = content.push(model_row);

            if !is_connected {
                content = content.push(
                    text("OFFLINE -- showing last known info")
                        .size(10)
                        .color(tc.orange),
                );
            }

            content = content.push(horizontal_rule(1));

            // Technical details
            let detail_rows = [
                ("Interface:", si.interface_type.as_str()),
                ("Firmware:", si.firmware.as_str()),
                ("Status:", si.status.as_str()),
            ];
            for (label, value) in &detail_rows {
                if !value.is_empty() {
                    content = content.push(
                        row![
                            text(*label).size(11).color(tc.text_sec),
                            text(*value).size(11).color(tc.text),
                        ]
                        .spacing(6),
                    );
                }
            }
            content = content.push(horizontal_rule(1));
        } else if let Some(kd) = kd {
            content = content.push(text(&kd.name).size(14).color(tc.text));
        }

        // Nickname editing
        let nickname_row = row![
            text("Nickname:").size(11).color(tc.text_sec),
            text_input("e.g. My 4TB Seagate", &self.nickname_buf)
                .on_input(Message::NicknameChanged)
                .width(200)
                .size(12),
            button(text("Save").size(11).color(tc.teal))
                .on_press(Message::SaveNickname)
                .padding([2, 8])
                .style(move |_theme: &Theme, status| {
                    let bg = match status {
                        button::Status::Hovered => blend_color(tc.bg_surface, tc.teal, 0.15),
                        button::Status::Pressed => blend_color(tc.bg_surface, tc.teal, 0.25),
                        _ => iced::Color::TRANSPARENT,
                    };
                    button::Style {
                        background: Some(iced::Background::Color(bg)),
                        border: iced::Border {
                            color: tc.teal,
                            width: 0.5,
                            radius: 3.0.into(),
                        },
                        text_color: tc.teal,
                        ..Default::default()
                    }
                }),
        ]
        .spacing(6)
        .align_y(iced::Alignment::Center);
        content = content.push(nickname_row);

        // Device info section
        content = content.push(Space::with_height(4));
        content = content.push(text(">DEVICE INFO").size(12).color(tc.cyan));

        if let Some(kd) = kd {
            let info_rows = [
                ("Device ID:", kd.device_id.as_str()),
                (
                    "VID:PID:",
                    if kd.vid_pid.is_empty() {
                        "-"
                    } else {
                        &kd.vid_pid
                    },
                ),
                ("Class:", &kd.class),
                (
                    "Manufacturer:",
                    if kd.manufacturer.is_empty() {
                        "-"
                    } else {
                        &kd.manufacturer
                    },
                ),
                (
                    "Description:",
                    if kd.description.is_empty() {
                        "-"
                    } else {
                        &kd.description
                    },
                ),
            ];
            for (label, value) in &info_rows {
                content = content.push(
                    row![
                        text(*label).size(11).color(tc.text_sec),
                        text(*value).size(11).color(tc.text),
                    ]
                    .spacing(6),
                );
            }

            // History
            content = content.push(horizontal_rule(1));
            content = content.push(text(">HISTORY").size(12).color(tc.cyan));
            content = content.push(
                row![
                    text(format!("First seen: {}", kd.first_seen))
                        .size(11)
                        .color(tc.text_sec),
                    text(format!("Last seen: {}", kd.last_seen))
                        .size(11)
                        .color(tc.text_sec),
                    text(format!("Times seen: {}x", kd.times_seen))
                        .size(11)
                        .color(tc.teal),
                ]
                .spacing(16),
            );
        }

        // Action buttons
        content = content.push(horizontal_rule(1));

        let dev_id_for_copy = device_id.to_string();
        let dev_id_for_forget = device_id.to_string();

        let mut action_row = Row::new().spacing(8);
        action_row = action_row.push(
            button(text("Copy ID").size(11).color(tc.text_sec))
                .on_press(Message::CopyToClipboard(dev_id_for_copy))
                .padding([2, 8])
                .style(move |_theme: &Theme, status| {
                    let bg = match status {
                        button::Status::Hovered => blend_color(tc.bg_surface, tc.accent, 0.15),
                        button::Status::Pressed => blend_color(tc.bg_surface, tc.accent, 0.25),
                        _ => iced::Color::TRANSPARENT,
                    };
                    button::Style {
                        background: Some(iced::Background::Color(bg)),
                        border: iced::Border {
                            color: tc.border,
                            width: 0.5,
                            radius: 3.0.into(),
                        },
                        text_color: tc.text_sec,
                        ..Default::default()
                    }
                }),
        );

        if let Some(si) = si {
            if !si.serial_number.is_empty() {
                let serial = si.serial_number.clone();
                action_row = action_row.push(
                    button(text("Copy Serial").size(11).color(tc.text_sec))
                        .on_press(Message::CopyToClipboard(serial))
                        .padding([2, 8])
                        .style(move |_theme: &Theme, status| {
                            let bg = match status {
                                button::Status::Hovered => {
                                    blend_color(tc.bg_surface, tc.accent, 0.15)
                                }
                                button::Status::Pressed => {
                                    blend_color(tc.bg_surface, tc.accent, 0.25)
                                }
                                _ => iced::Color::TRANSPARENT,
                            };
                            button::Style {
                                background: Some(iced::Background::Color(bg)),
                                border: iced::Border {
                                    color: tc.border,
                                    width: 0.5,
                                    radius: 3.0.into(),
                                },
                                text_color: tc.text_sec,
                                ..Default::default()
                            }
                        }),
                );
            }
        }

        action_row = action_row.push(
            button(text("\u{00D7} Forget").size(11).color(tc.red))
                .on_press(Message::ForgetDevice(dev_id_for_forget))
                .padding([2, 8])
                .style(move |_theme: &Theme, status| {
                    let bg = match status {
                        button::Status::Hovered => blend_color(tc.bg_surface, tc.red, 0.15),
                        button::Status::Pressed => blend_color(tc.bg_surface, tc.red, 0.25),
                        _ => iced::Color::TRANSPARENT,
                    };
                    button::Style {
                        background: Some(iced::Background::Color(bg)),
                        border: iced::Border {
                            color: tc.red,
                            width: 0.5,
                            radius: 3.0.into(),
                        },
                        text_color: tc.red,
                        ..Default::default()
                    }
                }),
        );

        content = content.push(action_row);

        let panel_bg = blend_color(tc.bg_surface, tc.accent, 0.03);
        let panel_border = blend_color(tc.border, tc.accent, 0.3);
        container(content)
            .padding(12)
            .width(Length::Fill)
            .style(move |_theme: &Theme| container::Style {
                background: Some(iced::Background::Color(panel_bg)),
                border: iced::Border {
                    color: panel_border,
                    width: 1.0,
                    radius: 6.0.into(),
                },
                ..Default::default()
            })
            .into()
    }
}

// ── Helpers ────────────────────────────────────────────────────

fn rainbow_separator<'a>(tc: &ThemeColors) -> Element<'a, Message> {
    let colors = [tc.red, tc.orange, tc.yellow, tc.green, tc.teal, tc.cyan, tc.accent, tc.pink];
    let mut rainbow_row = Row::new();
    for c in colors {
        rainbow_row = rainbow_row.push(
            container(Space::new(Length::Fill, 2))
                .width(Length::Fill)
                .style(move |_theme: &Theme| container::Style {
                    background: Some(iced::Background::Color(c)),
                    ..Default::default()
                }),
        );
    }
    rainbow_row.height(2).into()
}

fn blend_color(base: iced::Color, target: iced::Color, t: f32) -> iced::Color {
    iced::Color {
        r: base.r * (1.0 - t) + target.r * t,
        g: base.g * (1.0 - t) + target.g * t,
        b: base.b * (1.0 - t) + target.b * t,
        a: 1.0,
    }
}

async fn check_for_updates() -> Option<String> {
    let current = env!("CARGO_PKG_VERSION");
    let resp = ureq::get(
        "https://api.github.com/repos/TrentSterling/device-history/releases/latest",
    )
    .set("User-Agent", "device-history")
    .call()
    .ok()?;
    let body = resp.into_string().ok()?;
    let start = body.find("\"tag_name\"")?;
    let rest = &body[start..];
    let colon = rest.find(':')?;
    let after_colon = rest[colon + 1..].trim_start();
    if !after_colon.starts_with('"') {
        return None;
    }
    let val_end = after_colon[1..].find('"')?;
    let tag = &after_colon[1..1 + val_end];
    let latest = tag.trim_start_matches('v');
    if latest != current {
        Some(latest.to_string())
    } else {
        None
    }
}

// ── CLI mode ───────────────────────────────────────────────────

fn run_cli() {
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

    // ── Monitor thread ──
    let (monitor_tx, monitor_rx) = mpsc::channel();
    thread::spawn(move || monitor_loop(monitor_tx));

    // ── Tray event thread ──
    let (tray_tx, tray_rx) = mpsc::channel();
    let show_id = tray_menu_ids.show.clone();
    let hide_id = tray_menu_ids.hide.clone();
    let exit_id = tray_menu_ids.exit.clone();
    thread::spawn(move || loop {
        thread::sleep(Duration::from_millis(100));

        if let Ok(event) = MenuEvent::receiver().try_recv() {
            if event.id == show_id {
                let _ = tray_tx.send(TrayAction::Show);
            } else if event.id == hide_id {
                let _ = tray_tx.send(TrayAction::Hide);
            } else if event.id == exit_id {
                let _ = tray_tx.send(TrayAction::Exit);
            }
        }

        if let Ok(TrayIconEvent::DoubleClick { .. }) = TrayIconEvent::receiver().try_recv() {
            let _ = tray_tx.send(TrayAction::Show);
        }
    });

    // ── Window icon ──
    let window_icon = {
        let png_bytes = include_bytes!("../assets/icon.png");
        let img = image::load_from_memory(png_bytes)
            .expect("Failed to load window icon")
            .into_rgba8();
        let (w, h) = img.dimensions();
        iced::window::icon::from_rgba(img.into_raw(), w, h).ok()
    };

    // ── Launch Iced ──
    let mut app = iced::application(
        DeviceHistoryApp::title,
        DeviceHistoryApp::update,
        DeviceHistoryApp::view,
    )
    .subscription(DeviceHistoryApp::subscription)
    .theme(DeviceHistoryApp::theme)
    .window_size((720.0, 620.0));

    if let Some(icon) = window_icon {
        app = app.window(iced::window::Settings {
            icon: Some(icon),
            min_size: Some((420.0, 340.0).into()),
            ..Default::default()
        });
    }

    let result = app.run_with(move || DeviceHistoryApp::new(monitor_rx, tray_menu_ids, tray_rx));

    if let Err(e) = result {
        eprintln!("GUI error: {}", e);
    }
}
