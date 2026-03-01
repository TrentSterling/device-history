<script lang="ts">
  import { app } from '../../lib/stores/app.svelte';
  import type { DeviceEvent } from '../../lib/types';
  import DetailPanel from '../shared/DetailPanel.svelte';

  let { event }: { event: DeviceEvent } = $props();

  let isSelected = $derived(app.selectedDevice === event.device_id);
  let isConnect = $derived(event.kind === 'connect');
  let si = $derived(app.storageInfo[event.device_id] ?? null);

  function toggle() {
    app.selectDevice(isSelected ? null : event.device_id);
  }
</script>

<div class="event-wrapper anim-slide-in">
  <button
    class="card event-card"
    class:selected={isSelected}
    class:connect-card={isConnect}
    class:disconnect-card={!isConnect}
    onclick={toggle}
  >
    <span class="time">{event.timestamp}</span>
    <span class="kind-badge" class:connect={isConnect} class:disconnect={!isConnect}>
      {isConnect ? '🔌 CONNECT' : '⛔ DISCONNECT'}
    </span>
    <span class="name">{event.name}</span>
    {#if event.vid_pid}
      <span class="vid-pid">🏷️ {event.vid_pid}</span>
    {/if}
    {#if si}
      {#each si.volumes as vol}
        <span class="drive">💿 {vol.drive_letter}</span>
      {/each}
    {/if}
    <span class="class-badge">{event.class}</span>
    {#if event.manufacturer}
      <span class="mfr">🏭 {event.manufacturer}</span>
    {/if}
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
    align-items: center;
    gap: 8px;
    width: 100%;
    text-align: left;
  }
  .connect-card {
    border-left: 3px solid var(--green);
  }
  .disconnect-card {
    border-left: 3px solid var(--red);
  }
  .time {
    font-size: 11px;
    color: var(--text-muted);
    font-family: 'Cascadia Code', 'Consolas', monospace;
  }
  .kind-badge {
    font-size: 11px;
    font-weight: 700;
    padding: 1px 6px;
    border-radius: 4px;
    white-space: nowrap;
  }
  .kind-badge.connect {
    color: var(--green);
    background: color-mix(in srgb, var(--green) 12%, transparent);
  }
  .kind-badge.disconnect {
    color: var(--red);
    background: color-mix(in srgb, var(--red) 12%, transparent);
  }
  .name {
    font-size: 12px;
    color: var(--text);
    font-weight: 500;
  }
  .vid-pid {
    font-size: 10px;
    color: var(--yellow);
  }
  .drive {
    font-size: 11px;
    color: var(--green);
    font-weight: 500;
  }
  .class-badge {
    font-size: 10px;
    color: var(--accent);
    padding: 1px 5px;
    border-radius: 3px;
    background: var(--accent-glow);
  }
  .mfr {
    font-size: 10px;
    color: var(--text-sec);
  }
</style>
