use crate::logging::log_to_file;

#[tauri::command]
pub fn check_for_updates() -> Option<String> {
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
    log_to_file(&format!(
        "Update check: current={} latest={}",
        current, latest
    ));
    // Only show update if latest is actually newer (simple semver compare)
    let is_newer = || -> bool {
        let cur_parts: Vec<u32> = current.split('.').filter_map(|s| s.parse().ok()).collect();
        let lat_parts: Vec<u32> = latest.split('.').filter_map(|s| s.parse().ok()).collect();
        for i in 0..3 {
            let c = cur_parts.get(i).copied().unwrap_or(0);
            let l = lat_parts.get(i).copied().unwrap_or(0);
            if l > c { return true; }
            if l < c { return false; }
        }
        false
    };
    if is_newer() {
        Some(latest.to_string())
    } else {
        None
    }
}

#[tauri::command]
pub async fn copy_to_clipboard(
    app: tauri::AppHandle,
    text: String,
) -> Result<(), String> {
    use tauri_plugin_clipboard_manager::ClipboardExt;
    app.clipboard()
        .write_text(&text)
        .map_err(|e| e.to_string())
}

#[tauri::command]
#[allow(deprecated)]
pub async fn open_url(
    app: tauri::AppHandle,
    url: String,
) -> Result<(), String> {
    use tauri_plugin_shell::ShellExt;
    app.shell()
        .open(&url, None)
        .map_err(|e| e.to_string())
}
