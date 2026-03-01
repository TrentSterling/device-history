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

</script>

<div class="device-wrapper">
  <button
    class="card device-card magnetic-hover"
    class:selected={isSelected}
    onclick={toggle}
  >
    <!-- Row 1: Status dot + Device name + Drive pills -->
    <div class="card-row">
      <div class="row-left">
        <span class="status-dot online"></span>
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
    {#if kd?.nickname}
      <div class="nickname">"{kd.nickname}"</div>
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
    flex-direction: column;
    gap: 3px;
    width: 100%;
    text-align: left;
    padding: 12px 16px;
    border-left: 3px solid var(--green);
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
</style>
