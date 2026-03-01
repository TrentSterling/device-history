<script lang="ts">
  import { app } from '../../lib/stores/app.svelte';
  import { relativeDate } from '../../lib/utils';
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
    if (c.includes('bluetooth')) return '\uD83D\uDD35';
    if (c.includes('scsi') || c.includes('disk')) return '\uD83D\uDCBF';
    if (c.includes('hid') || c.includes('keyboard')) return '\u2328\uFE0F';
    if (c.includes('mouse')) return '\uD83D\uDDB1\uFE0F';
    if (c.includes('audio') || c.includes('sound')) return '\uD83D\uDD0A';
    if (c.includes('camera') || c.includes('video')) return '\uD83D\uDCF7';
    if (c.includes('net') || c.includes('wireless')) return '\uD83D\uDCF6';
    if (c.includes('print')) return '\uD83D\uDDA8\uFE0F';
    return '\uD83D\uDD0C';
  }
</script>

<div class="known-wrapper">
  <button
    class="card known-card magnetic-hover"
    class:selected={isSelected}
    class:connected={device.currently_connected}
    onclick={toggle}
  >
    <!-- Row 1: Status dot + Device name + Drive pills -->
    <div class="card-row">
      <div class="row-left">
        <span class="status-dot" class:online={device.currently_connected} class:offline={!device.currently_connected}></span>
        <span class="device-name">{device.name}</span>
      </div>
      {#if si}
        <span class="drive-pills">
          {#each si.volumes as vol}
            <span class="drive-pill">{vol.drive_letter}</span>
          {/each}
        </span>
      {/if}
    </div>

    <!-- Row 2: Nickname -->
    {#if device.nickname}
      <div class="nickname">"{device.nickname}"</div>
    {/if}

    <!-- Row 3: VID:PID + Class + Manufacturer -->
    <div class="meta-secondary">
      {#if device.vid_pid}
        <span>{device.vid_pid}</span>
        <span class="meta-dot">&middot;</span>
      {/if}
      <span>{device.class}</span>
      {#if device.manufacturer}
        <span class="meta-dot">&middot;</span>
        <span>{device.manufacturer}</span>
      {/if}
    </div>

    <!-- Row 4: History -->
    <div class="meta-history">
      <span>First: {relativeDate(device.first_seen)}</span>
      <span class="meta-dot">&middot;</span>
      <span>Last: {relativeDate(device.last_seen)}</span>
      <span class="meta-dot">&middot;</span>
      <span class="times-seen">{device.times_seen}&times; seen</span>
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
    padding: 12px 16px;
  }
  .known-card.connected {
    border-left: 3px solid var(--green);
    background: color-mix(in srgb, var(--bg-elevated) 92%, var(--green));
  }
  .card-row {
    display: flex;
    align-items: center;
    justify-content: space-between;
  }
  .row-left {
    display: flex;
    align-items: center;
    gap: 8px;
    min-width: 0;
  }
  .device-name {
    font-size: 14px;
    font-weight: 600;
    color: var(--text);
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  .drive-pills {
    display: flex;
    gap: 4px;
    flex-shrink: 0;
    margin-left: 8px;
  }
  .drive-pill {
    font-size: 11px;
    font-weight: 600;
    color: var(--green);
    background: color-mix(in srgb, var(--green) 10%, transparent);
    padding: 1px 6px;
    border-radius: 4px;
  }
  .nickname {
    font-size: 12px;
    color: var(--teal);
    margin-top: 2px;
    padding-left: 16px;
  }
  .meta-secondary {
    font-size: 11px;
    color: var(--text-muted);
    margin-top: 3px;
    padding-left: 16px;
  }
  .meta-dot {
    margin: 0 4px;
    opacity: 0.4;
  }
  .meta-history {
    font-size: 11px;
    color: var(--text-muted);
    margin-top: 4px;
    padding-left: 16px;
  }
  .times-seen {
    color: var(--teal);
    font-weight: 500;
  }
</style>
