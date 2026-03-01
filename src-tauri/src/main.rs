// Hide console window in GUI mode (release builds only)
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

fn main() {
    let args: Vec<String> = std::env::args().collect();
    if args.iter().any(|a| a == "--cli") {
        device_history_lib::run_cli_mode();
        return;
    }
    device_history_lib::run();
}
