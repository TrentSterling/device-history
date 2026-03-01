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
