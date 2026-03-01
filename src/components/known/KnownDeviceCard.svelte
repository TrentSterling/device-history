<script lang="ts">
  import { app } from '../../lib/stores/app.svelte';
  import type { KnownDevice } from '../../lib/types';
  import DetailPanel from '../shared/DetailPanel.svelte';

  let { device }: { device: KnownDevice } = $props();

  let isSelected = $derived(app.selectedDevice === device.device_id);
  let si = $derived(app.getStorageForDevice(device.device_id));

  function toggle() {
    app.selectDevice(isSelected ? null : device.device_id);
  }

  function classEmoji(cls: string): string {
    const c = cls.toLowerCase();
    if (c.includes('bluetooth')) return '🔵';
    if (c.includes('scsi') || c.includes('disk')) return '💿';
    if (c.includes('hid') || c.includes('keyboard')) return '⌨️';
    if (c.includes('mouse')) return '🖱️';
    if (c.includes('audio') || c.includes('sound')) return '🔊';
    if (c.includes('camera') || c.includes('video')) return '📷';
    if (c.includes('net') || c.includes('wireless')) return '📶';
    if (c.includes('print')) return '🖨️';
    return '🔌';
  }
</script>

<div class="known-wrapper">
  <button
    class="card known-card"
    class:selected={isSelected}
    class:connected={device.currently_connected}
    onclick={toggle}
  >
    <div class="row1">
      <span class="status-dot" class:online={device.currently_connected} class:offline={!device.currently_connected}></span>
      {#if si}
        {#each si.volumes as vol}
          <span class="drive">💿 {vol.drive_letter}</span>
          {#if vol.volume_name}
            <span class="vol-name">"{vol.volume_name}"</span>
          {/if}
        {/each}
        {#if si.model}
          <span class="model">{si.model}</span>
        {/if}
      {:else}
        <span class="class-icon">{classEmoji(device.class)}</span>
        <span class="class-badge">{device.class}</span>
        <span class="name">{device.name}</span>
      {/if}
      {#if device.nickname}
        <span class="nickname">✨ {device.nickname}</span>
      {/if}
      {#if device.vid_pid}
        <span class="vid-pid">🏷️ {device.vid_pid}</span>
      {/if}
      {#if !si && device.manufacturer}
        <span class="mfr">{device.manufacturer}</span>
      {/if}
    </div>
    <div class="row2">
      <span>📅 First: {device.first_seen}</span>
      <span>🕐 Last: {device.last_seen}</span>
      <span class="times-seen">🔄 {device.times_seen}x</span>
    </div>
  </button>
  {#if isSelected}
    <DetailPanel deviceId={device.device_id} isConnected={device.currently_connected} />
  {/if}
</div>

<style>
  .known-wrapper {
    display: flex;
    flex-direction: column;
    gap: 4px;
  }
  .known-card {
    display: flex;
    flex-direction: column;
    gap: 3px;
    width: 100%;
    text-align: left;
  }
  .known-card.connected {
    border-left: 3px solid var(--green);
    background: color-mix(in srgb, var(--bg-elevated) 92%, var(--green));
  }
  .row1 {
    display: flex;
    align-items: center;
    gap: 6px;
  }
  .row2 {
    display: flex;
    gap: 12px;
    font-size: 10px;
    color: var(--text-muted);
    padding-left: 2px;
  }
  .status-dot {
    width: 8px;
    height: 8px;
    border-radius: 50%;
    flex-shrink: 0;
  }
  .status-dot.online {
    background: var(--green);
    box-shadow: 0 0 6px var(--green);
    animation: glow-pulse 2s ease-in-out infinite;
  }
  .status-dot.offline {
    background: var(--text-muted);
    opacity: 0.4;
  }
  .class-icon {
    font-size: 13px;
    flex-shrink: 0;
  }
  .drive {
    font-size: 13px;
    color: var(--green);
    font-weight: 600;
  }
  .vol-name {
    font-size: 12px;
    color: var(--text);
  }
  .model {
    font-size: 11px;
    color: var(--text-sec);
  }
  .class-badge {
    font-size: 10px;
    color: var(--accent);
    padding: 1px 5px;
    border-radius: 3px;
    background: var(--accent-glow);
  }
  .name {
    font-size: 12px;
    color: var(--text);
    font-weight: 500;
  }
  .nickname {
    font-size: 11px;
    color: var(--teal);
    font-weight: 500;
  }
  .vid-pid {
    font-size: 10px;
    color: var(--yellow);
    opacity: 0.8;
  }
  .mfr {
    font-size: 10px;
    color: var(--text-muted);
  }
  .times-seen {
    color: var(--teal);
    font-weight: 500;
  }
</style>
