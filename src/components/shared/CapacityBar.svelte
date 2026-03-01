<script lang="ts">
  import { usedPercent, capacityColor, formatBytes } from '../../lib/utils';

  let { total, free }: { total: number; free: number } = $props();

  let pct = $derived(usedPercent(total, free));
  let color = $derived(capacityColor(pct));
</script>

{#if total > 0}
  <div class="capacity-bar">
    <div class="capacity-fill" style="width: {pct}%; --bar-color: {color};"></div>
    <div class="capacity-shimmer"></div>
  </div>
  <div class="capacity-text" style="color: {color}">
    {formatBytes(free)} free / {formatBytes(total)} ({pct.toFixed(0)}% used)
  </div>
{/if}

<style>
  .capacity-bar {
    height: 14px;
    background: color-mix(in srgb, var(--bg-deep) 80%, transparent);
    border-radius: 7px;
    overflow: hidden;
    position: relative;
    border: 1px solid var(--border);
  }
  .capacity-fill {
    height: 100%;
    background: linear-gradient(90deg, var(--bar-color), color-mix(in srgb, var(--bar-color) 70%, white));
    border-radius: 7px;
    transition: width 600ms cubic-bezier(0.22, 1, 0.36, 1);
    position: relative;
    box-shadow: 0 0 8px color-mix(in srgb, var(--bar-color) 30%, transparent);
  }
  .capacity-shimmer {
    position: absolute;
    top: 0;
    left: 0;
    right: 0;
    bottom: 0;
    background: linear-gradient(
      90deg,
      transparent 0%,
      color-mix(in srgb, white 12%, transparent) 50%,
      transparent 100%
    );
    background-size: 200% 100%;
    animation: shimmer 3s ease-in-out infinite;
    border-radius: 7px;
    pointer-events: none;
  }
  .capacity-text {
    font-size: 11px;
    margin-top: 3px;
    font-weight: 500;
    font-family: 'Cascadia Code', 'Consolas', monospace;
  }

  @keyframes shimmer {
    0% { background-position: 200% 0; }
    100% { background-position: -200% 0; }
  }
</style>
