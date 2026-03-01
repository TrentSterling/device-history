use crate::types::KnownDeviceCache;

const CACHE_FILE: &str = "device-history-cache.json";

pub fn load_cache() -> KnownDeviceCache {
    std::fs::read_to_string(CACHE_FILE)
        .ok()
        .and_then(|s| serde_json::from_str(&s).ok())
        .unwrap_or_else(KnownDeviceCache::new)
}

pub fn save_cache(cache: &KnownDeviceCache) {
    if let Ok(json) = serde_json::to_string_pretty(cache) {
        let _ = std::fs::write(CACHE_FILE, json);
    }
}
