use serde::{Deserialize, Serialize};
use std::collections::HashMap;

// ── WMI device struct ──────────────────────────────────────────

#[derive(Deserialize, Debug, Clone)]
#[allow(non_snake_case)]
pub struct UsbDevice {
    pub Name: Option<String>,
    pub DeviceID: Option<String>,
    pub Description: Option<String>,
    pub Manufacturer: Option<String>,
    pub PNPClass: Option<String>,
}

impl UsbDevice {
    pub fn display_name(&self) -> &str {
        self.Name
            .as_deref()
            .or(self.Description.as_deref())
            .unwrap_or("Unknown Device")
    }

    pub fn vid_pid(&self) -> Option<String> {
        let id = self.DeviceID.as_ref()?;
        let vid_start = id.find("VID_").map(|i| i + 4)?;
        let vid = id.get(vid_start..vid_start + 4)?;
        let pid_start = id.find("PID_").map(|i| i + 4)?;
        let pid = id.get(pid_start..pid_start + 4)?;
        Some(format!("{}:{}", vid, pid))
    }

    pub fn class(&self) -> &str {
        self.PNPClass.as_deref().unwrap_or("?")
    }
}

// ── Storage info ───────────────────────────────────────────────

#[derive(Deserialize, Debug, Clone)]
#[allow(non_snake_case)]
pub struct WmiDiskDrive {
    pub DeviceID: Option<String>,
    pub PNPDeviceID: Option<String>,
    pub Model: Option<String>,
    pub SerialNumber: Option<String>,
    pub Size: Option<u64>,
    pub InterfaceType: Option<String>,
    pub MediaType: Option<String>,
    pub Partitions: Option<u32>,
    pub FirmwareRevision: Option<String>,
    pub Status: Option<String>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct StorageInfo {
    pub model: String,
    pub serial_number: String,
    pub total_bytes: u64,
    pub interface_type: String,
    pub media_type: String,
    pub firmware: String,
    pub partition_count: u32,
    pub status: String,
    pub volumes: Vec<VolumeInfo>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct VolumeInfo {
    pub drive_letter: String,
    pub volume_name: String,
    pub total_bytes: u64,
    pub free_bytes: u64,
    pub file_system: String,
    pub volume_serial: String,
}

// ── Known device cache ─────────────────────────────────────────

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct KnownDevice {
    pub device_id: String,
    pub name: String,
    pub vid_pid: String,
    pub class: String,
    pub manufacturer: String,
    pub description: String,
    pub first_seen: String,
    pub last_seen: String,
    pub times_seen: u32,
    pub currently_connected: bool,
    #[serde(default)]
    pub nickname: Option<String>,
    #[serde(default)]
    pub storage_info: Option<StorageInfo>,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct KnownDeviceCache {
    pub version: u32,
    pub devices: HashMap<String, KnownDevice>,
}

impl KnownDeviceCache {
    pub fn new() -> Self {
        Self {
            version: 2,
            devices: HashMap::new(),
        }
    }
}

// ── Device event ───────────────────────────────────────────────

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct DeviceEvent {
    pub timestamp: String,
    pub kind: String, // "connect" or "disconnect"
    pub name: String,
    pub vid_pid: Option<String>,
    pub manufacturer: Option<String>,
    pub class: String,
    pub device_id: String,
}

// ── Snapshot (sent to frontend) ────────────────────────────────

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct DeviceSnapshot {
    pub device_id: String,
    pub name: String,
    pub vid_pid: Option<String>,
    pub manufacturer: Option<String>,
    pub class: String,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct AppSnapshot {
    pub devices: Vec<DeviceSnapshot>,
    pub events: Vec<DeviceEvent>,
    pub known_devices: HashMap<String, KnownDevice>,
    pub storage_info: HashMap<String, StorageInfo>,
    pub error: Option<String>,
}

// ── Preferences ────────────────────────────────────────────────

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Prefs {
    pub theme: String,
    pub active_tab: String,
}

impl Default for Prefs {
    fn default() -> Self {
        Self {
            theme: "neon".to_string(),
            active_tab: "monitor".to_string(),
        }
    }
}
