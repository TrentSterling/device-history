export function formatBytes(bytes: number): string {
  const KB = 1024;
  const MB = 1024 * KB;
  const GB = 1024 * MB;
  const TB = 1024 * GB;
  if (bytes >= TB) return `${(bytes / TB).toFixed(2)} TB`;
  if (bytes >= GB) return `${(bytes / GB).toFixed(2)} GB`;
  if (bytes >= MB) return `${(bytes / MB).toFixed(1)} MB`;
  if (bytes >= KB) return `${(bytes / KB).toFixed(0)} KB`;
  return `${bytes} B`;
}

export function formatTimestamp(ts: string): string {
  return ts;
}

export function usedPercent(total: number, free: number): number {
  if (total === 0) return 0;
  return ((1 - free / total) * 100);
}

export function capacityColor(pct: number): string {
  if (pct < 70) return 'var(--green)';
  if (pct < 90) return 'var(--yellow)';
  return 'var(--red)';
}

export function relativeDate(isoString: string): string {
  const date = new Date(isoString);
  const now = new Date();
  const diffMs = now.getTime() - date.getTime();
  const diffDays = Math.floor(diffMs / (1000 * 60 * 60 * 24));

  if (diffDays === 0) return "Today";
  if (diffDays === 1) return "Yesterday";
  if (diffDays < 7) return `${diffDays}d ago`;
  if (diffDays < 30) return `${Math.floor(diffDays / 7)}w ago`;

  return date.toLocaleDateString("en-US", { month: "short", day: "numeric" });
}

export function deviceClassCategory(className: string | undefined): string {
  if (!className) return "Other";
  const c = className.toLowerCase();
  if (c.includes("disk") || c.includes("storage") || c.includes("cdrom")) return "Storage";
  if (c.includes("hid") || c.includes("keyboard") || c.includes("mouse")) return "HID";
  if (c.includes("audio") || c.includes("sound")) return "Audio";
  if (c.includes("bluetooth")) return "Bluetooth";
  if (c.includes("net") || c.includes("wireless")) return "Network";
  return "Other";
}

export type DeviceClassFilter = "All" | "Storage" | "HID" | "Audio" | "Bluetooth" | "Network" | "Other";

export const CLASS_FILTERS: DeviceClassFilter[] = ["All", "Storage", "HID", "Audio", "Bluetooth", "Network", "Other"];
