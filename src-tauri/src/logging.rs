use chrono::Local;
use std::fs::OpenOptions;
use std::io::Write;

pub fn log_to_file(msg: &str) {
    let path = "device-history.log";
    if let Ok(mut f) = OpenOptions::new().create(true).append(true).open(path) {
        let ts = Local::now().format("%Y-%m-%d %H:%M:%S%.3f");
        let _ = writeln!(f, "[{}] {}", ts, msg);
    }
}
