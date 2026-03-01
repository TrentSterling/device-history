mod cache;
mod cli;
mod commands;
mod logging;
mod monitor;
mod state;
mod storage;
mod types;

use state::AppState;

pub fn run_cli_mode() {
    cli::run_cli();
}
use std::sync::Arc;
use tauri::{
    Manager,
    tray::{MouseButton, MouseButtonState, TrayIconBuilder, TrayIconEvent},
    menu::{Menu, MenuItem, PredefinedMenuItem},
    WindowEvent,
};

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    let prefs = commands::prefs::load_initial_prefs();
    let app_state = Arc::new(AppState::new(prefs.theme, prefs.active_tab));

    tauri::Builder::default()
        .plugin(tauri_plugin_clipboard_manager::init())
        .plugin(tauri_plugin_shell::init())
        .manage(app_state.clone())
        .invoke_handler(tauri::generate_handler![
            commands::snapshot::get_snapshot,
            commands::nicknames::set_nickname,
            commands::nicknames::forget_device,
            commands::events::clear_events,
            commands::prefs::get_prefs,
            commands::prefs::set_theme,
            commands::prefs::set_tab,
            commands::system::check_for_updates,
            commands::system::copy_to_clipboard,
            commands::system::open_url,
        ])
        .setup(move |app| {
            // ── System tray ──
            let show_item = MenuItem::with_id(app, "show", "Show", true, None::<&str>)?;
            let hide_item = MenuItem::with_id(app, "hide", "Hide", true, None::<&str>)?;
            let separator = PredefinedMenuItem::separator(app)?;
            let exit_item = MenuItem::with_id(app, "exit", "Exit", true, None::<&str>)?;

            let menu = Menu::with_items(app, &[&show_item, &hide_item, &separator, &exit_item])?;

            let _tray = TrayIconBuilder::new()
                .menu(&menu)
                .tooltip("Device History")
                .on_menu_event(|app, event| {
                    match event.id.as_ref() {
                        "show" => {
                            if let Some(win) = app.get_webview_window("main") {
                                let _ = win.show();
                                let _ = win.unminimize();
                                let _ = win.set_focus();
                            }
                        }
                        "hide" => {
                            if let Some(win) = app.get_webview_window("main") {
                                let _ = win.hide();
                            }
                        }
                        "exit" => {
                            std::process::exit(0);
                        }
                        _ => {}
                    }
                })
                .on_tray_icon_event(|tray, event| {
                    if let TrayIconEvent::Click {
                        button: MouseButton::Left,
                        button_state: MouseButtonState::Up,
                        ..
                    } = event
                    {
                        let app = tray.app_handle();
                        if let Some(win) = app.get_webview_window("main") {
                            let _ = win.show();
                            let _ = win.unminimize();
                            let _ = win.set_focus();
                        }
                    }
                })
                .build(app)?;

            // ── Start monitor thread ──
            let handle = app.handle().clone();
            monitor::start_monitor(handle, app_state);

            Ok(())
        })
        .on_window_event(|window, event| {
            if let WindowEvent::CloseRequested { api, .. } = event {
                api.prevent_close();
                let _ = window.hide();
            }
        })
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
