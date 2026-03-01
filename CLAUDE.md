# CLAUDE.md — Device History

## Project
**Device History** v0.8.0 — Real-time USB device monitor for Windows.
"WTF just disconnected?"

**Owner:** Trent Sterling (tront.xyz)
**Path:** `C:\trontstack\device-history`
**Repo:** https://github.com/TrentSterling/device-history
**Landing:** https://tront.xyz/device-history/

## Tech Stack
- **Backend:** Rust (Tauri v2, WMI polling, serde, chrono)
- **Frontend:** Svelte 5, TypeScript, Vite
- **GUI Framework:** Tauri v2 (WebView2)
- **Device Detection:** WMI `Win32_PnPEntity` + `Win32_DiskDrive` queries
- **Storage:** JSON file persistence via Tauri commands

## Architecture

```
src/                    Svelte 5 frontend
├── App.svelte          Root — keyboard shortcuts, theme class, tab routing
├── app.css             Global styles (3 themes, animations, glassmorphism)
├── lib/
│   ├── stores/
│   │   └── app.svelte.ts   Reactive state (AppState class, $state/$derived)
│   ├── commands.ts     Tauri invoke wrappers
│   ├── themes.ts       Theme metadata (Neon, Dracula, Mocha)
│   ├── types.ts        TypeScript interfaces
│   └── utils.ts        relativeDate, deviceClassCategory, formatBytes, etc.
├── components/
│   ├── Header.svelte           Logo, theme pills, status
│   ├── Footer.svelte           Stats bar, CSV export, sound toggle
│   ├── TabBar.svelte           Monitor / Known Devices tabs
│   ├── monitor/
│   │   ├── EventLog.svelte     Event list with scroll shadow
│   │   ├── EventCard.svelte    Connect/disconnect event card
│   │   ├── ConnectedDevices.svelte  Currently connected device list
│   │   └── ConnectedDeviceCard.svelte
│   ├── known/
│   │   ├── KnownDevicesTab.svelte   Search + sort + filter + device list
│   │   ├── KnownDeviceCard.svelte   Device card with history stats
│   │   ├── SearchBar.svelte
│   │   └── SortControls.svelte      Sort pills (Status/Name/Last/Count/First)
│   └── shared/
│       ├── DetailPanel.svelte  Single flat card — all device data (no tabs)
│       ├── CapacityBar.svelte  Animated storage bar with shimmer
│       ├── ClassFilter.svelte  Filter pills (All/Storage/HID/Audio/Bluetooth/Network/Other)
│       └── Toasts.svelte       Toast notification stack

src-tauri/src/          Rust backend
├── main.rs             Entry point
├── lib.rs              Tauri command registrations
├── monitor.rs          WMI polling thread (500ms interval)
├── state.rs            AppState, KnownDevice, event tracking
├── storage.rs          Disk drive/volume info via WMI
├── cache.rs            JSON file persistence
├── types.rs            Shared types
├── logging.rs          File logging
└── cli.rs              CLI mode (--cli flag)
```

## Themes (3)
- **Neon** — default, vivid purple accent, dark
- **Dracula** — classic Dracula palette
- **Mocha** — Catppuccin Mocha, softer/warmer

CSS variables in `app.css`, metadata in `lib/themes.ts`.

## Key Patterns

### Svelte 5 Reactivity
- `AppState` class uses `$state` for reactive properties
- Getters (`get filteredKnown()`) auto-track reactive reads
- Components use `$derived` for local computed values
- `$effect` for side effects (e.g., resetting tab state)

### Tauri IPC
Frontend calls Rust via `invoke()` wrappers in `commands.ts`.
Rust pushes updates via `device-update` event (listened in `app.svelte.ts`).

### Device Detection Flow
1. Rust `monitor.rs` polls WMI every 500ms on background thread
2. Diffs current devices against previous snapshot
3. Emits `device-update` event with full `AppSnapshot`
4. Frontend `applySnapshot()` updates all reactive state

## Dev Commands

```bash
npm install              # Install frontend deps
npx tauri dev            # Dev mode (hot reload frontend + Rust backend)
npx tauri build          # Production build
npm run dev              # Vite dev server only (no Rust)
npm run build            # Vite build only
```

## Icons
All icons in `src-tauri/icons/` must be **square** PNGs.
- `32x32.png`, `128x128.png`, `128x128@2x.png` (256x256), `icon.png` (256x256)
- `icon.ico` — multi-size ICO (16/24/32/48/64/128/256)
- Tray icon uses `icon.ico` (configured in `tauri.conf.json`)

## Gotchas
- **Icons must be square** — non-square PNGs cause invisible taskbar/tray icons
- **Tray icon on Windows** — use `.ico` format, not `.png`
- **Port 1420** — Vite dev server; kill stale processes if "port in use" error
- **eframe legacy** — v0.4.0 was eframe, v0.5.0+ is Tauri v2 + Svelte 5
- **Sort directions** — "ascending" shows best-first (connected, most recent, most seen)
