import { listen } from "@tauri-apps/api/event";
import * as cmd from "../commands";
import { deviceClassCategory, type DeviceClassFilter } from "../utils";
import type {
  AppSnapshot,
  DeviceEvent,
  DeviceSnapshot,
  KnownDevice,
  StorageInfo,
} from "../types";

export type SortMode = "status" | "name" | "last_seen" | "times_seen" | "first_seen";

class AppState {
  // Data
  devices = $state<DeviceSnapshot[]>([]);
  events = $state<DeviceEvent[]>([]);
  knownDevices = $state<Record<string, KnownDevice>>({});
  storageInfo = $state<Record<string, StorageInfo>>({});
  error = $state<string | null>(null);

  // UI state
  theme = $state("neon");
  activeTab = $state<"monitor" | "known">("monitor");
  classFilter = $state<DeviceClassFilter>("All");
  soundEnabled = $state(false);
  isLoading = $state(true);
  searchQuery = $state("");
  sortMode = $state<SortMode>("status");
  sortAscending = $state(true);
  selectedDevice = $state<string | null>(null);
  nicknameBuf = $state("");

  // System
  updateAvailable = $state<string | null>(null);

  // Notifications
  notifications = $state<{ id: number; text: string; kind: string }[]>([]);
  private nextNotifId = 0;

  // Derived: filtered events by class
  get filteredEvents(): DeviceEvent[] {
    if (this.classFilter === "All") return this.events;
    return this.events.filter(e => deviceClassCategory(e.class) === this.classFilter);
  }

  // Derived: filtered + sorted known devices
  get filteredKnown(): KnownDevice[] {
    const q = this.searchQuery.toLowerCase();
    let list = Object.values(this.knownDevices);

    if (q) {
      list = list.filter(
        (d) =>
          d.name.toLowerCase().includes(q) ||
          d.device_id.toLowerCase().includes(q) ||
          d.class.toLowerCase().includes(q) ||
          d.manufacturer.toLowerCase().includes(q) ||
          d.vid_pid.toLowerCase().includes(q) ||
          (d.nickname ?? "").toLowerCase().includes(q)
      );
    }

    if (this.classFilter !== "All") {
      list = list.filter(d => deviceClassCategory(d.class) === this.classFilter);
    }

    const mode = this.sortMode;
    const asc = this.sortAscending;
    list.sort((a, b) => {
      let cmp = 0;
      switch (mode) {
        case "status":
          // Connected first (descending bool), then alphabetical
          cmp = Number(b.currently_connected) - Number(a.currently_connected);
          if (cmp === 0)
            cmp = a.name.toLowerCase().localeCompare(b.name.toLowerCase());
          break;
        case "name":
          cmp = a.name.toLowerCase().localeCompare(b.name.toLowerCase());
          break;
        case "last_seen":
          // Most recent first
          cmp = b.last_seen.localeCompare(a.last_seen);
          break;
        case "times_seen":
          // Most-seen first
          cmp = b.times_seen - a.times_seen;
          break;
        case "first_seen":
          // Most recent first
          cmp = b.first_seen.localeCompare(a.first_seen);
          break;
      }
      return asc ? cmp : -cmp;
    });

    return list;
  }

  get knownTotal(): number {
    return Object.keys(this.knownDevices).length;
  }

  get knownOnline(): number {
    return Object.values(this.knownDevices).filter((d) => d.currently_connected)
      .length;
  }

  async init() {
    // Load initial snapshot
    try {
      const snap = await cmd.getSnapshot();
      this.applySnapshot(snap);
    } catch (e) {
      console.error("Failed to get initial snapshot:", e);
    }

    this.isLoading = false;

    // Load prefs
    try {
      const prefs = await cmd.getPrefs();
      const validThemes = ["neon", "dracula", "mocha"];
      this.theme = validThemes.includes(prefs.theme) ? prefs.theme : "neon";
      this.activeTab = prefs.active_tab === "known" ? "known" : "monitor";
    } catch (e) {
      console.error("Failed to load prefs:", e);
    }

    // Check for updates
    try {
      const ver = await cmd.checkForUpdates();
      this.updateAvailable = ver;
    } catch {
      // Silently ignore
    }

    // Listen for real-time updates from monitor thread
    listen<AppSnapshot>("device-update", (event) => {
      const prevCount = this.events.length;
      this.applySnapshot(event.payload);

      // Show toast and play sound for new connect/disconnect events
      const newEvents = this.events.slice(prevCount);
      if (newEvents.length > 0 && this.soundEnabled) {
        try {
          const ctx = new AudioContext();
          const osc = ctx.createOscillator();
          const gain = ctx.createGain();
          osc.connect(gain);
          gain.connect(ctx.destination);
          osc.frequency.value = 880;
          osc.type = "sine";
          gain.gain.setValueAtTime(0.15, ctx.currentTime);
          gain.gain.exponentialRampToValueAtTime(0.001, ctx.currentTime + 0.15);
          osc.start();
          osc.stop(ctx.currentTime + 0.15);
        } catch {}
      }
      for (const evt of newEvents.slice(-3)) {
        const icon = evt.kind === "connect" ? "\u{1F50C}" : "\u23CF\uFE0F";
        const verb = evt.kind === "connect" ? "Connected" : "Disconnected";
        this.notify(`${icon} ${verb}: ${evt.name || "USB Device"}`, evt.kind === "connect" ? "success" : "error");
      }
    });
  }

  private applySnapshot(snap: AppSnapshot) {
    this.devices = snap.devices;
    this.events = snap.events;
    this.knownDevices = snap.known_devices;
    this.storageInfo = snap.storage_info;
    if (snap.error) this.error = snap.error;
  }

  selectDevice(id: string | null) {
    if (id && this.knownDevices[id]) {
      this.nicknameBuf = this.knownDevices[id].nickname ?? "";
    } else {
      this.nicknameBuf = "";
    }
    this.selectedDevice = id;
  }

  async saveNickname() {
    if (!this.selectedDevice) return;
    await cmd.setNickname(this.selectedDevice, this.nicknameBuf);
    // Update local state
    const dev = this.knownDevices[this.selectedDevice];
    if (dev) {
      dev.nickname = this.nicknameBuf.trim() || null;
      this.knownDevices = { ...this.knownDevices };
    }
    this.notify("Nickname saved", "success");
  }

  async forgetDevice(id: string) {
    await cmd.forgetDevice(id);
    const updated = { ...this.knownDevices };
    delete updated[id];
    this.knownDevices = updated;
    const si = { ...this.storageInfo };
    delete si[id];
    this.storageInfo = si;
    if (this.selectedDevice === id) this.selectedDevice = null;
    this.notify("Device forgotten", "info");
  }

  async clearEvents() {
    await cmd.clearEvents();
    this.events = [];
  }

  async setTheme(id: string) {
    this.theme = id;
    await cmd.setTheme(id);
  }

  async setActiveTab(tab: "monitor" | "known") {
    this.activeTab = tab;
    this.selectedDevice = null;
    await cmd.setTab(tab);
  }

  toggleSort(mode: SortMode) {
    if (this.sortMode === mode) {
      this.sortAscending = !this.sortAscending;
    } else {
      this.sortMode = mode;
      this.sortAscending = true;
    }
  }

  setClassFilter(filter: DeviceClassFilter) {
    this.classFilter = filter;
  }

  toggleSound() {
    this.soundEnabled = !this.soundEnabled;
  }

  exportEventsCSV() {
    const header = "Timestamp,Event,Name,VID:PID,Class,Manufacturer,DeviceID";
    const rows = this.events.map(e =>
      `"${e.timestamp}","${e.kind}","${e.name || ""}","${e.vid_pid || ""}","${e.class || ""}","${e.manufacturer || ""}","${e.device_id}"`
    );
    const csv = [header, ...rows].join("\n");
    const blob = new Blob([csv], { type: "text/csv" });
    const url = URL.createObjectURL(blob);
    const a = document.createElement("a");
    a.href = url;
    a.download = `device-history-${new Date().toISOString().slice(0, 10)}.csv`;
    a.click();
    URL.revokeObjectURL(url);
    this.notify("Events exported to CSV", "success");
  }

  notify(text: string, kind: string = "info") {
    const id = this.nextNotifId++;
    this.notifications = [...this.notifications, { id, text, kind }];
    setTimeout(() => {
      this.notifications = this.notifications.filter((n) => n.id !== id);
    }, 2500);
  }

  async copyToClipboard(text: string) {
    await cmd.copyToClipboard(text);
    this.notify("Copied to clipboard", "success");
  }

  async openUrl(url: string) {
    await cmd.openUrl(url);
  }

  getStorageForDevice(deviceId: string): StorageInfo | null {
    return (
      this.storageInfo[deviceId] ??
      this.knownDevices[deviceId]?.storage_info ??
      null
    );
  }
}

export const app = new AppState();
