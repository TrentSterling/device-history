# Device History

**WTF just disconnected?**

Real-time USB device monitor for Windows. Watches for connect/disconnect events via WMI polling and logs everything with timestamps. Built with Rust + Tauri v2 + Svelte 5.

![Device History v0.8.0](screenshot.png)

## Features

- **Live monitoring** — 500ms WMI poll, instant connect/disconnect detection
- **Event log** — timestamped history with color-coded cards
- **Device database** — remembers every device ever connected with first/last seen, connection count
- **Storage info** — capacity bars, model, serial, firmware for disk drives
- **Device nicknames** — label your devices for easy identification
- **3 themes** — Neon, Dracula, Catppuccin Mocha with smooth transitions
- **Class filtering** — filter by Storage, HID, Audio, Bluetooth, Network
- **Search & sort** — find devices by name, VID:PID, class, manufacturer
- **CSV export** — export event log as CSV
- **Sound notifications** — optional audio beep on connect/disconnect
- **Keyboard shortcuts** — Escape to close, 1/2 to switch tabs
- **System tray** — minimize to tray, background monitoring
- **CLI mode** — `--cli` flag for terminal output
- **File logging** — persistent log at `device-history.log`
- **Update checker** — checks GitHub releases on startup

## Install

Download the latest installer from [Releases](https://github.com/TrentSterling/device-history/releases/latest).

Or build from source:

```bash
git clone https://github.com/TrentSterling/device-history
cd device-history
npm install
npx tauri build
```

## Usage

```bash
# GUI mode (default)
device-history

# CLI mode
device-history --cli
```

## Tech Stack

- **Rust** + **Tauri v2** — backend, WMI queries, system tray
- **Svelte 5** + **TypeScript** — reactive frontend
- **Vite** — build tooling
- **WMI** — Windows Management Instrumentation for device detection

## How It Works

1. Rust backend polls `Win32_PnPEntity` via WMI every 500ms on a background thread
2. Diffs against the previous snapshot to detect connects/disconnects
3. Pushes `device-update` events to the Svelte frontend via Tauri
4. Frontend renders device cards, event log, storage info with glassmorphism UI
5. Known devices are persisted to a JSON database file

## License

MIT

---

Made by [tront](https://tront.xyz) | [Landing Page](https://tront.xyz/device-history/)
