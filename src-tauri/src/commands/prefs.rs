use crate::state::AppState;
use crate::types::Prefs;
use std::sync::Arc;
use tauri::State;

const PREFS_FILE: &str = "device-history.prefs";

fn load_prefs() -> Prefs {
    let Ok(content) = std::fs::read_to_string(PREFS_FILE) else {
        return Prefs::default();
    };
    let mut prefs = Prefs::default();
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

fn save_prefs(prefs: &Prefs) {
    let content = format!("theme={}\nactive_tab={}\n", prefs.theme, prefs.active_tab);
    let _ = std::fs::write(PREFS_FILE, content);
}

pub fn load_initial_prefs() -> Prefs {
    load_prefs()
}

#[tauri::command]
pub fn get_prefs(state: State<'_, Arc<AppState>>) -> Prefs {
    Prefs {
        theme: state.prefs_theme.read().clone(),
        active_tab: state.prefs_tab.read().clone(),
    }
}

#[tauri::command]
pub fn set_theme(state: State<'_, Arc<AppState>>, theme: String) {
    *state.prefs_theme.write() = theme.clone();
    let tab = state.prefs_tab.read().clone();
    save_prefs(&Prefs {
        theme,
        active_tab: tab,
    });
}

#[tauri::command]
pub fn set_tab(state: State<'_, Arc<AppState>>, tab: String) {
    *state.prefs_tab.write() = tab.clone();
    let theme = state.prefs_theme.read().clone();
    save_prefs(&Prefs {
        theme,
        active_tab: tab,
    });
}
