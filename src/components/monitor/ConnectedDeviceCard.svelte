<script lang="ts">
  import { app } from '../../lib/stores/app.svelte';
  import type { DeviceSnapshot } from '../../lib/types';
  import DetailPanel from '../shared/DetailPanel.svelte';

  let { device }: { device: DeviceSnapshot } = $props();

  let isSelected = $derived(app.selectedDevice === device.device_id);
  let si = $derived(app.storageInfo[device.device_id] ?? null);
  let kd = $derived(app.knownDevices[device.device_id] ?? null);

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

<div class="device-wrapper">
  <button
    class="card device-card"
    class:selected={isSelected}
    onclick={toggle}
  >
    <span class="status-dot online"></span>
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
    {#if kd?.nickname}
      <span class="nickname">✨ {kd.nickname}</span>
    {/if}
    {#if device.vid_pid}
      <span class="vid-pid">[{device.vid_pid}]</span>
    {/if}
    {#if !si && device.manufacturer}
      <span class="mfr">{device.manufacturer}</span>
    {/if}
  </button>
  {#if isSelected}
    <DetailPanel deviceId={device.device_id} isConnected={true} />
  {/if}
</div>

<style>
  .device-wrapper {
    display: flex;
    flex-direction: column;
    gap: 4px;
  }
  .device-card {
    display: flex;
    align-items: center;
    gap: 8px;
    width: 100%;
    text-align: left;
    border-left: 3px solid var(--green);
  }
  .class-icon {
    font-size: 14px;
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
</style>
