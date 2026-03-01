export interface VolumeInfo {
  drive_letter: string;
  volume_name: string;
  total_bytes: number;
  free_bytes: number;
  file_system: string;
  volume_serial: string;
}

export interface StorageInfo {
  model: string;
  serial_number: string;
  total_bytes: number;
  interface_type: string;
  media_type: string;
  firmware: string;
  partition_count: number;
  status: string;
  volumes: VolumeInfo[];
}

export interface KnownDevice {
  device_id: string;
  name: string;
  vid_pid: string;
  class: string;
  manufacturer: string;
  description: string;
  first_seen: string;
  last_seen: string;
  times_seen: number;
  currently_connected: boolean;
  nickname: string | null;
  storage_info: StorageInfo | null;
}

export interface DeviceEvent {
  timestamp: string;
  kind: "connect" | "disconnect";
  name: string;
  vid_pid: string | null;
  manufacturer: string | null;
  class: string;
  device_id: string;
}

export interface DeviceSnapshot {
  device_id: string;
  name: string;
  vid_pid: string | null;
  manufacturer: string | null;
  class: string;
}

export interface AppSnapshot {
  devices: DeviceSnapshot[];
  events: DeviceEvent[];
  known_devices: Record<string, KnownDevice>;
  storage_info: Record<string, StorageInfo>;
  error: string | null;
}

export interface Prefs {
  theme: string;
  active_tab: string;
}
