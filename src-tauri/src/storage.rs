use crate::logging::log_to_file;
use crate::types::{StorageInfo, UsbDevice, VolumeInfo, WmiDiskDrive};
use serde::Deserialize;
use wmi::WMIConnection;

pub fn is_storage_device(dev: &UsbDevice) -> bool {
    let class = dev.PNPClass.as_deref().unwrap_or("");
    let name = dev.Name.as_deref().unwrap_or("");
    class.contains("SCSIAdapter")
        || class.contains("DiskDrive")
        || (class.contains("USB") && name.contains("Storage"))
        || name.contains("Mass Storage")
}

pub fn format_bytes(bytes: u64) -> String {
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

pub fn query_storage_info(wmi: &WMIConnection, device_id: &str) -> Option<StorageInfo> {
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
