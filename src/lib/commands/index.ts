import { invoke } from "@tauri-apps/api/core";
import type { AppSnapshot, Prefs } from "../types";

export async function getSnapshot(): Promise<AppSnapshot> {
  return invoke("get_snapshot");
}

export async function setNickname(deviceId: string, nickname: string): Promise<void> {
  return invoke("set_nickname", { deviceId, nickname });
}

export async function forgetDevice(deviceId: string): Promise<void> {
  return invoke("forget_device", { deviceId });
}

export async function clearEvents(): Promise<void> {
  return invoke("clear_events");
}

export async function getPrefs(): Promise<Prefs> {
  return invoke("get_prefs");
}

export async function setTheme(theme: string): Promise<void> {
  return invoke("set_theme", { theme });
}

export async function setTab(tab: string): Promise<void> {
  return invoke("set_tab", { tab });
}

export async function checkForUpdates(): Promise<string | null> {
  return invoke("check_for_updates");
}

export async function copyToClipboard(text: string): Promise<void> {
  return invoke("copy_to_clipboard", { text });
}

export async function openUrl(url: string): Promise<void> {
  return invoke("open_url", { url });
}
