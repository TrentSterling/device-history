use crate::state::AppState;
use std::sync::Arc;
use tauri::State;

#[tauri::command]
pub fn clear_events(state: State<'_, Arc<AppState>>) {
    state.events.write().clear();
}
