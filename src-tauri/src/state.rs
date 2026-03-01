use crate::types::{AppSnapshot, DeviceEvent, DeviceSnapshot, KnownDevice, StorageInfo};
use parking_lot::RwLock;
use std::collections::HashMap;

pub struct AppState {
    pub devices: RwLock<Vec<DeviceSnapshot>>,
    pub events: RwLock<Vec<DeviceEvent>>,
    pub known_devices: RwLock<HashMap<String, KnownDevice>>,
    pub storage_info: RwLock<HashMap<String, StorageInfo>>,
    pub error: RwLock<Option<String>>,
    pub prefs_theme: RwLock<String>,
    pub prefs_tab: RwLock<String>,
}

impl AppState {
    pub fn new(theme: String, tab: String) -> Self {
        Self {
            devices: RwLock::new(Vec::new()),
            events: RwLock::new(Vec::new()),
            known_devices: RwLock::new(HashMap::new()),
            storage_info: RwLock::new(HashMap::new()),
            error: RwLock::new(None),
            prefs_theme: RwLock::new(theme),
            prefs_tab: RwLock::new(tab),
        }
    }

    pub fn snapshot(&self) -> AppSnapshot {
        AppSnapshot {
            devices: self.devices.read().clone(),
            events: self.events.read().clone(),
            known_devices: self.known_devices.read().clone(),
            storage_info: self.storage_info.read().clone(),
            error: self.error.read().clone(),
        }
    }
}
