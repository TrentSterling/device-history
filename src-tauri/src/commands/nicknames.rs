use crate::cache::save_cache;
use crate::state::AppState;
use crate::types::KnownDeviceCache;
use std::sync::Arc;
use tauri::State;

#[tauri::command]
pub fn set_nickname(
    state: State<'_, Arc<AppState>>,
    device_id: String,
    nickname: String,
) {
    let mut known = state.known_devices.write();
    if let Some(dev) = known.get_mut(&device_id) {
        dev.nickname = if nickname.trim().is_empty() {
            None
        } else {
            Some(nickname.trim().to_string())
        };
        let cache = KnownDeviceCache {
            version: 2,
            devices: known.clone(),
        };
        save_cache(&cache);
    }
}

#[tauri::command]
pub fn forget_device(
    state: State<'_, Arc<AppState>>,
    device_id: String,
) {
    let mut known = state.known_devices.write();
    known.remove(&device_id);
    state.storage_info.write().remove(&device_id);
    let cache = KnownDeviceCache {
        version: 2,
        devices: known.clone(),
    };
    save_cache(&cache);
}
