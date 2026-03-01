<script lang="ts">
  import { app } from '../../lib/stores/app.svelte';
  import type { DeviceEvent } from '../../lib/types';
  import DetailPanel from '../shared/DetailPanel.svelte';

  let { event }: { event: DeviceEvent } = $props();

  let isSelected = $derived(app.selectedDevice === event.device_id);
  let isConnect = $derived(event.kind === 'connect');
  let si = $derived(app.storageInfo[event.device_id] ?? null);
</script>

<div class="event-wrapper anim-slide-in">
  <button
    class="card event-card magnetic-hover"
    class:selected={isSelected}
    class:connect-card={isConnect}
    class:disconnect-card={!isConnect}
    onclick={() => app.selectDevice(isSelected ? null : event.device_id)}
  >
    <!-- Row 1: Badge + Timestamp -->
    <div class="card-row">
      <span class="event-badge" class:connect={isConnect} class:disconnect={!isConnect}>
        {isConnect ? '\u25B2 CONNECT' : '\u25BC DISCONNECT'}
      </span>
      <span class="event-time">{event.timestamp}</span>
    </div>

    <!-- Row 2: Device name + Drive pills -->
    <div class="card-row">
      <span class="device-name">{event.name}</span>
      {#if si}
        <span class="drive-pills">
          {#each si.volumes as vol}
            <span class="drive-pill">{vol.drive_letter}</span>
          {/each}
        </span>
      {/if}
    </div>

    <!-- Row 3: VID:PID + Class -->
    <div class="meta-secondary">
      {#if event.vid_pid}
        <span>{event.vid_pid}</span>
        <span class="meta-dot">&middot;</span>
      {/if}
      <span>{event.class}</span>
    </div>
  </button>
  {#if isSelected}
    <DetailPanel deviceId={event.device_id} isConnected={isConnect} />
  {/if}
</div>

<style>
  .event-wrapper {
    display: flex;
    flex-direction: column;
    gap: 4px;
  }
  .event-card {
    display: flex;
    flex-direction: column;
    gap: 4px;
    width: 100%;
    text-align: left;
    padding: 12px 16px;
  }
  .connect-card {
    border-left: 3px solid var(--green);
  }
  .disconnect-card {
    border-left: 3px solid var(--red);
  }
  .card-row {
    display: flex;
    align-items: center;
    justify-content: space-between;
  }
  .event-badge {
    font-size: 11px;
    font-weight: 700;
    padding: 2px 8px;
    border-radius: 4px;
    white-space: nowrap;
  }
  .event-badge.connect {
    color: var(--green);
    background: color-mix(in srgb, var(--green) 12%, transparent);
  }
  .event-badge.disconnect {
    color: var(--red);
    background: color-mix(in srgb, var(--red) 12%, transparent);
  }
  .event-time {
    font-family: "Cascadia Code", "Consolas", monospace;
    font-size: 11px;
    color: var(--text-muted);
  }
  .device-name {
    font-size: 14px;
    font-weight: 600;
    color: var(--text);
    min-width: 0;
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
  .meta-secondary {
    font-size: 11px;
    color: var(--text-muted);
    margin-top: 3px;
  }
  .meta-dot {
    margin: 0 4px;
    opacity: 0.4;
  }
</style>
