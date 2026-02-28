# Device History

**WTF just disconnected?**

Real-time USB device monitor for Windows with a neon-themed GUI and CLI mode. Watches for connect/disconnect events via WMI polling and logs everything with timestamps.

![Device History v0.3.0](screenshot.png)

## Features

- **Live monitoring** — 500ms poll interval, instant connect/disconnect detection
- **Event log** — timestamped history with color-coded cards (green = connect, red = disconnect)
- **Device details** — VID:PID, class, manufacturer for every USB device
- **File logging** — persistent log at `device-history.log`
- **CLI mode** — `--cli` flag for terminal output with colored text
- **Theme picker** — Neon (dark), Light, and Mids themes
- **Rainbow gradient** — because why not

## Install

```bash
cargo install --git https://github.com/TrentSterling/device-history
```

Or build from source:

```bash
git clone https://github.com/TrentSterling/device-history
cd device-history
cargo build --release
```

Binary lands at `target/release/device-history.exe`.

## Usage

```bash
# GUI mode (default)
device-history

# CLI mode
device-history --cli
```

## Tech Stack

- **Rust** — fast, safe, single binary
- **eframe/egui** — immediate-mode GUI
- **WMI** — Windows Management Instrumentation for device queries
- **chrono** — timestamps
- **colored** — CLI terminal colors

## How It Works

1. Queries `Win32_PnPEntity` via WMI for all USB devices
2. Snapshots the device list every 500ms on a background thread
3. Diffs against the previous snapshot to detect connects/disconnects
4. Logs events to file and displays them in the GUI with neon-styled cards
5. Each event card has a colored left accent bar and shows device name, VID:PID, class, and manufacturer

## License

MIT

---

Made by [tront](https://tront.xyz)
