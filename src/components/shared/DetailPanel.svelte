<script lang="ts">
  import { app } from '../../lib/stores/app.svelte';
  import { relativeDate } from '../../lib/utils';
  import CapacityBar from './CapacityBar.svelte';

  let { deviceId, isConnected }: { deviceId: string; isConnected: boolean } = $props();

  let kd = $derived(app.knownDevices[deviceId] ?? null);
  let si = $derived(app.getStorageForDevice(deviceId));

  let deviceEvents = $derived(
    app.events
      .filter(e => e.device_id === deviceId)
      .slice(-30)
  );
</script>

<div class="detail-panel">
  <button class="close-btn" onclick={() => app.selectDevice(null)}>&#10005;</button>

  <!-- Nickname -->
  <div class="nickname-row">
    <input
      type="text"
      placeholder="e.g. My 4TB Seagate"
      bind:value={app.nicknameBuf}
    />
    <button class="action-btn" onclick={() => app.saveNickname()}>Save</button>
  </div>

  <!-- Metadata -->
  {#if kd}
    <div class="info-grid">
      <span class="info-label">Name</span>
      <span class="info-value">{kd.name}</span>

      {#if kd.vid_pid}
        <span class="info-label">VID:PID</span>
        <span class="info-value">{kd.vid_pid}</span>
      {/if}

      <span class="info-label">Class</span>
      <span class="info-value">{kd.class}</span>

      {#if kd.manufacturer}
        <span class="info-label">Manufacturer</span>
        <span class="info-value">{kd.manufacturer}</span>
      {/if}

      {#if kd.description}
        <span class="info-label">Description</span>
        <span class="info-value">{kd.description}</span>
      {/if}
    </div>
  {/if}

  <!-- Storage -->
  {#if si}
    {#if !isConnected}
      <div class="offline-notice">Offline — storage info may be stale</div>
    {/if}

    {#each si.volumes as vol}
      <div class="volume-card">
        <div class="volume-header">
          <span class="drive-letter">{vol.drive_letter}</span>
          {#if vol.volume_name}
            <span class="volume-name">{vol.volume_name}</span>
          {/if}
          <span class="fs-badge">{vol.file_system}</span>
        </div>
        <CapacityBar total={vol.total_bytes} free={vol.free_bytes} />
      </div>
    {/each}

    <div class="info-grid">
      {#if si.model}
        <span class="info-label">Model</span>
        <span class="info-value">{si.model}</span>
      {/if}
      {#if si.serial_number}
        <span class="info-label">Serial</span>
        <span class="info-value">{si.serial_number}</span>
      {/if}
      {#if si.interface_type}
        <span class="info-label">Interface</span>
        <span class="info-value">{si.interface_type}</span>
      {/if}
      {#if si.firmware}
        <span class="info-label">Firmware</span>
        <span class="info-value">{si.firmware}</span>
      {/if}
    </div>
  {/if}

  <!-- History -->
  {#if kd}
    <div class="history-row">
      <span class="history-item">First: <strong>{relativeDate(kd.first_seen)}</strong></span>
      <span class="dot">&middot;</span>
      <span class="history-item">Last: <strong>{relativeDate(kd.last_seen)}</strong></span>
      <span class="dot">&middot;</span>
      <span class="history-item times-seen">{kd.times_seen}&times; seen</span>
    </div>

    {#if deviceEvents.length > 0}
      <div class="sparkline">
        {#each deviceEvents as evt}
          <div
            class="spark-dot {evt.kind === 'connect' ? 'connect' : 'disconnect'}"
            title="{evt.kind} — {evt.timestamp}"
          ></div>
        {/each}
      </div>
    {/if}
  {/if}

  <!-- Device ID + Actions -->
  {#if kd}
    <div class="device-id-row">
      <span class="device-id-text">{kd.device_id}</span>
      <button class="action-btn" onclick={() => app.copyToClipboard(kd!.device_id)}>Copy</button>
    </div>
  {/if}

  <div class="action-row">
    {#if si?.serial_number}
      <button class="action-btn" onclick={() => app.copyToClipboard(si!.serial_number)}>Copy Serial</button>
    {/if}
    <button class="action-btn danger" onclick={() => app.forgetDevice(deviceId)}>Forget Device</button>
  </div>
</div>

<style>
  .detail-panel {
    background: var(--bg-surface);
    border: 1px solid var(--border);
    border-top: none;
    padding: 12px 14px;
    display: flex;
    flex-direction: column;
    gap: 10px;
    animation: expandHeight 350ms cubic-bezier(0.34, 1.56, 0.64, 1);
    position: relative;
  }

  .close-btn {
    position: absolute;
    top: 8px;
    right: 8px;
    color: var(--text-muted);
    font-size: 14px;
    padding: 4px 8px;
    border-radius: 4px;
    z-index: 1;
  }
  .close-btn:hover {
    color: var(--red);
    background: color-mix(in srgb, var(--red) 10%, transparent);
  }

  .nickname-row {
    display: flex;
    gap: 8px;
    align-items: center;
    padding-right: 28px;
  }
  .nickname-row input {
    flex: 1;
    padding: 5px 10px;
    background: var(--bg-deep);
    border: 1px solid var(--border);
    border-radius: 6px;
    color: var(--teal);
    font-size: 12px;
    outline: none;
    transition: border-color 150ms;
  }
  .nickname-row input:focus {
    border-color: var(--teal);
    box-shadow: 0 0 0 2px color-mix(in srgb, var(--teal) 20%, transparent);
  }
  .nickname-row input::placeholder {
    color: var(--text-muted);
    font-size: 11px;
  }

  .info-grid {
    display: grid;
    grid-template-columns: auto 1fr;
    gap: 4px 12px;
    font-size: 12px;
  }
  .info-label {
    color: var(--text-muted);
    font-weight: 500;
  }
  .info-value {
    color: var(--text);
    word-break: break-all;
  }

  .volume-card {
    background: var(--bg-elevated);
    border: 1px solid var(--border);
    border-radius: 8px;
    padding: 10px 12px;
  }
  .volume-header {
    display: flex;
    align-items: center;
    gap: 8px;
    margin-bottom: 6px;
  }
  .drive-letter {
    font-size: 15px;
    font-weight: 700;
    color: var(--green);
  }
  .volume-name {
    font-size: 13px;
    color: var(--text);
  }
  .fs-badge {
    font-size: 10px;
    color: var(--text-muted);
    background: var(--bg-deep);
    padding: 2px 6px;
    border-radius: 4px;
    margin-left: auto;
  }

  .history-row {
    font-size: 11px;
    color: var(--text-sec);
    display: flex;
    align-items: center;
    gap: 6px;
    flex-wrap: wrap;
  }
  .history-row .dot {
    opacity: 0.4;
  }
  .times-seen {
    color: var(--teal);
    font-weight: 600;
  }

  .sparkline {
    display: flex;
    gap: 3px;
    align-items: center;
    padding: 8px 10px;
    background: var(--bg-deep);
    border-radius: 6px;
    border: 1px solid var(--border);
    overflow-x: auto;
  }
  .spark-dot {
    width: 6px;
    height: 6px;
    border-radius: 50%;
    flex-shrink: 0;
  }
  .spark-dot.connect {
    background: var(--green);
  }
  .spark-dot.disconnect {
    background: var(--red);
  }

  .device-id-row {
    display: flex;
    align-items: center;
    gap: 8px;
    padding: 6px 8px;
    background: var(--bg-deep);
    border-radius: 6px;
    border: 1px solid var(--border);
  }
  .device-id-text {
    font-family: "Cascadia Code", "Consolas", monospace;
    font-size: 10px;
    color: var(--text-muted);
    flex: 1;
    word-break: break-all;
  }

  .offline-notice {
    padding: 6px 10px;
    background: color-mix(in srgb, var(--orange) 10%, transparent);
    border: 1px solid color-mix(in srgb, var(--orange) 30%, transparent);
    border-radius: 6px;
    color: var(--orange);
    font-size: 11px;
  }

  .action-row {
    display: flex;
    gap: 8px;
    flex-wrap: wrap;
  }
  .action-btn {
    padding: 5px 10px;
    border-radius: 6px;
    border: 1px solid var(--border);
    background: none;
    color: var(--text-sec);
    font-size: 11px;
    cursor: pointer;
    transition: all 150ms;
  }
  .action-btn:hover {
    border-color: var(--accent);
    color: var(--accent);
  }
  .action-btn.danger {
    border-color: color-mix(in srgb, var(--red) 30%, transparent);
    color: var(--red);
  }
  .action-btn.danger:hover {
    background: color-mix(in srgb, var(--red) 10%, transparent);
    border-color: var(--red);
  }
</style>
