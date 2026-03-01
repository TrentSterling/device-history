use crate::cache::{load_cache, save_cache};
use crate::logging::log_to_file;
use crate::state::AppState;
use crate::storage::{is_storage_device, query_storage_info};
use crate::types::{DeviceEvent, DeviceSnapshot, KnownDevice, StorageInfo, UsbDevice};
use chrono::Local;
use std::collections::HashMap;
use std::sync::Arc;
use std::thread;
use std::time::{Duration, Instant};
use tauri::{AppHandle, Emitter};
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

fn usb_to_snapshot(id: &str, dev: &UsbDevice) -> DeviceSnapshot {
    DeviceSnapshot {
        device_id: id.to_string(),
        name: dev.display_name().to_string(),
        vid_pid: dev.vid_pid(),
        manufacturer: dev.Manufacturer.clone(),
        class: dev.class().to_string(),
    }
}

pub fn start_monitor(app_handle: AppHandle, state: Arc<AppState>) {
    thread::spawn(move || {
        monitor_loop(app_handle, state);
    });
}

fn emit_update(app_handle: &AppHandle, state: &AppState) {
    let snapshot = state.snapshot();
    let _ = app_handle.emit("device-update", &snapshot);
}

fn monitor_loop(app_handle: AppHandle, state: Arc<AppState>) {
    let com = match COMLibrary::new() {
        Ok(c) => c,
        Err(e) => {
            *state.error.write() = Some(format!("COM init failed: {}", e));
            emit_update(&app_handle, &state);
            return;
        }
    };
    let wmi = match WMIConnection::new(com) {
        Ok(w) => w,
        Err(e) => {
            *state.error.write() = Some(format!("WMI connect failed: {}", e));
            emit_update(&app_handle, &state);
            return;
        }
    };

    let mut prev = match query_devices(&wmi) {
        Some(d) => d,
        None => {
            *state.error.write() = Some("Failed to query USB devices".into());
            emit_update(&app_handle, &state);
            return;
        }
    };

    let mut known_cache = load_cache();
    let mut storage_map: HashMap<String, StorageInfo> = HashMap::new();
    let mut all_events: Vec<DeviceEvent> = Vec::new();

    // Initial snapshot — merge into cache
    {
        let now = Local::now().format("%Y-%m-%d %H:%M:%S").to_string();
        for dev in known_cache.devices.values_mut() {
            dev.currently_connected = false;
        }
        for (id, dev) in &prev {
            let is_new = !known_cache.devices.contains_key(id);
            let entry = known_cache
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
        save_cache(&known_cache);
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
                storage_map.insert(id.clone(), info.clone());
                if let Some(kd) = known_cache.devices.get_mut(id) {
                    kd.storage_info = Some(info);
                }
                save_cache(&known_cache);
            }
        }
    }

    // Push initial state
    {
        let mut sorted: Vec<_> = prev.iter().map(|(id, d)| usb_to_snapshot(id, d)).collect();
        sorted.sort_by(|a, b| a.name.to_lowercase().cmp(&b.name.to_lowercase()));
        *state.devices.write() = sorted;
        *state.known_devices.write() = known_cache.devices.clone();
        *state.storage_info.write() = storage_map.clone();
        emit_update(&app_handle, &state);
    }

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
                storage_map.insert(enrich_id.clone(), info.clone());
                if let Some(kd) = known_cache.devices.get_mut(&enrich_id) {
                    kd.storage_info = Some(info);
                }
                save_cache(&known_cache);
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
                    kind: "disconnect".to_string(),
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
                    kind: "connect".to_string(),
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
                .filter(|e| e.kind == "connect")
                .filter(|e| {
                    current
                        .get(&e.device_id)
                        .map_or(false, |d| is_storage_device(d))
                })
                .map(|e| e.device_id.clone())
                .collect();

            for event in &new_events {
                match event.kind.as_str() {
                    "connect" => {
                        if let Some(dev) = current.get(&event.device_id) {
                            let is_new =
                                !known_cache.devices.contains_key(&event.device_id);
                            let entry = known_cache
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
                                entry.manufacturer = dev.Manufacturer.clone().unwrap_or_default();
                                entry.description = dev.Description.clone().unwrap_or_default();
                            }
                        }
                    }
                    "disconnect" => {
                        if let Some(entry) = known_cache.devices.get_mut(&event.device_id) {
                            entry.last_seen = now_iso.clone();
                            entry.currently_connected = false;
                        }
                        storage_map.remove(&event.device_id);
                    }
                    _ => {}
                }
            }

            all_events.extend(new_events);
            save_cache(&known_cache);

            for id in enrich_ids {
                pending_enrichments.push((id, Instant::now()));
            }
        }

        // Check if we need to update the known_devices from external changes (nickname, forget)
        // We re-read cache periodically to pick up command-side mutations
        {
            let cmd_known = state.known_devices.read().clone();
            // Sync: if a device was forgotten via command, remove from our cache too
            let our_ids: Vec<String> = known_cache.devices.keys().cloned().collect();
            for id in &our_ids {
                if !cmd_known.contains_key(id) {
                    known_cache.devices.remove(id);
                    storage_map.remove(id);
                    save_cache(&known_cache);
                }
            }
            // Sync nicknames from commands
            for (id, cmd_dev) in &cmd_known {
                if let Some(our_dev) = known_cache.devices.get_mut(id) {
                    if our_dev.nickname != cmd_dev.nickname {
                        our_dev.nickname = cmd_dev.nickname.clone();
                        save_cache(&known_cache);
                    }
                }
            }
        }

        let has_changes = !all_events.is_empty() || enriched || prev.len() != current.len();

        if has_changes {
            let mut sorted: Vec<_> = current
                .iter()
                .map(|(id, d)| usb_to_snapshot(id, d))
                .collect();
            sorted.sort_by(|a, b| a.name.to_lowercase().cmp(&b.name.to_lowercase()));

            *state.devices.write() = sorted;
            *state.events.write() = all_events.clone();
            *state.known_devices.write() = known_cache.devices.clone();
            *state.storage_info.write() = storage_map.clone();
            emit_update(&app_handle, &state);
        }

        prev = current;
    }
}
