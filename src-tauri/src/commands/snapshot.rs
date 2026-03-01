use crate::state::AppState;
use crate::types::AppSnapshot;
use std::sync::Arc;
use tauri::State;

#[tauri::command]
pub fn get_snapshot(state: State<'_, Arc<AppState>>) -> AppSnapshot {
    state.snapshot()
}
