<script lang="ts">
  import { app } from '../../lib/stores/app.svelte';
  import { relativeDate } from '../../lib/utils';
  import CapacityBar from './CapacityBar.svelte';

  let { deviceId, isConnected }: { deviceId: string; isConnected: boolean } = $props();

  let kd = $derived(app.knownDevices[deviceId] ?? null);
  let si = $derived(app.getStorageForDevice(deviceId));
  let hasStorage = $derived(!!si);

  type DetailTab = 'info' | 'storage' | 'history';
  let activeTab = $state<DetailTab>('info');

  // Reset to info tab if storage disappears while on storage tab
  $effect(() => {
    if (activeTab === 'storage' && !hasStorage) {
      activeTab = 'info';
    }
  });

  // Device events for sparkline (most recent 30)
  let deviceEvents = $derived(
    app.events
      .filter(e => e.device_id === deviceId)
      .slice(-30)
  );
</script>

<div class="detail-panel">
  <!-- Tab bar -->
  <div class="detail-tabs">
    <button
      class:active={activeTab === 'info'}
      onclick={() => activeTab = 'info'}
    >Info</button>
    {#if hasStorage}
      <button
        class:active={activeTab === 'storage'}
        onclick={() => activeTab = 'storage'}
      >Storage</button>
    {/if}
    <button
      class:active={activeTab === 'history'}
      onclick={() => activeTab = 'history'}
    >History</button>
    <button class="close-btn" onclick={() => app.selectDevice(null)}>&#10005;</button>
  </div>

  <!-- Tab content -->
  {#key activeTab}
    <div class="detail-content">

      {#if activeTab === 'info'}
        <!-- Nickname -->
        <div class="nickname-row">
          <input
            type="text"
            placeholder="e.g. My 4TB Seagate"
            bind:value={app.nicknameBuf}
          />
          <button class="action-btn" onclick={() => app.saveNickname()}>Save</button>
        </div>

        <!-- Device metadata grid -->
        {#if kd}
          <div class="info-grid">
            <span class="info-label">Name</span>
            <span class="info-value">{kd.name}</span>

            <span class="info-label">VID:PID</span>
            <span class="info-value">{kd.vid_pid || '\u2014'}</span>

            <span class="info-label">Class</span>
            <span class="info-value">{kd.class}</span>

            <span class="info-label">Manufacturer</span>
            <span class="info-value">{kd.manufacturer || '\u2014'}</span>

            <span class="info-label">Description</span>
            <span class="info-value">{kd.description || '\u2014'}</span>
          </div>

          <!-- Device ID row -->
          <div class="device-id-row">
            <span class="device-id-text">{kd.device_id}</span>
            <button class="action-btn" onclick={() => app.copyToClipboard(kd!.device_id)}>Copy</button>
          </div>
        {/if}

        <!-- Actions -->
        <div class="action-row">
          {#if si?.serial_number}
            <button class="action-btn" onclick={() => app.copyToClipboard(si!.serial_number)}>Copy Serial</button>
          {/if}
          <button class="action-btn danger" onclick={() => app.forgetDevice(deviceId)}>Forget Device</button>
        </div>

      {:else if activeTab === 'storage' && si}
        <!-- Offline notice -->
        {#if !isConnected}
          <div class="offline-notice">Device is offline — storage info may be stale</div>
        {/if}

        <!-- Volume cards -->
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

        <!-- Drive metadata -->
        <div class="info-grid" style="margin-top: 8px;">
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

      {:else if activeTab === 'history'}
        {#if kd}
          <!-- Stats row -->
          <div class="history-stats">
            <div class="history-stat">
              <span class="value">{relativeDate(kd.first_seen)}</span>
              <span class="label" title={kd.first_seen}>First seen</span>
            </div>
            <div class="history-stat">
              <span class="value">{relativeDate(kd.last_seen)}</span>
              <span class="label" title={kd.last_seen}>Last seen</span>
            </div>
            <div class="history-stat">
              <span class="value">{kd.times_seen}</span>
              <span class="label">Times seen</span>
            </div>
          </div>

          <!-- Sparkline -->
          {#if deviceEvents.length > 0}
            <div class="sparkline">
              {#each deviceEvents as evt}
                <div
                  class="spark-dot {evt.kind === 'connect' ? 'connect' : 'disconnect'}"
                  title="{evt.kind} — {evt.timestamp}"
                ></div>
              {/each}
            </div>
          {:else}
            <div class="no-history">No event history recorded</div>
          {/if}
        {/if}
      {/if}
    </div>
  {/key}
</div>

<style>
  .detail-panel {
    background: var(--bg-surface);
    border: 1px solid var(--border);
    border-top: none;
    animation: expandHeight 350ms cubic-bezier(0.34, 1.56, 0.64, 1);
  }

  .detail-tabs {
    display: flex;
    gap: 0;
    border-bottom: 1px solid var(--border);
    padding: 0 12px;
    background: var(--bg-elevated);
  }

  .detail-tabs button {
    padding: 8px 16px;
    background: none;
    border: none;
    color: var(--text-muted);
    cursor: pointer;
    font-size: 12px;
    font-weight: 600;
    border-bottom: 2px solid transparent;
    transition: color 150ms, border-color 150ms;
  }

  .detail-tabs button.active {
    color: var(--accent);
    border-bottom-color: var(--accent);
  }

  .detail-tabs button:hover:not(.active) {
    color: var(--text);
  }

  .close-btn {
    margin-left: auto !important;
    color: var(--text-muted);
    font-size: 16px;
    padding: 8px 12px !important;
  }

  .close-btn:hover {
    color: var(--red);
  }

  .detail-content {
    padding: 14px;
    animation: crossfadeIn 150ms ease;
  }

  .info-grid {
    display: grid;
    grid-template-columns: auto 1fr;
    gap: 6px 12px;
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

  .device-id-row {
    display: flex;
    align-items: center;
    gap: 8px;
    margin-top: 10px;
    padding: 8px;
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

  .nickname-row {
    display: flex;
    gap: 8px;
    align-items: center;
    margin-bottom: 12px;
  }

  .nickname-row input {
    flex: 1;
    padding: 6px 10px;
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

  .volume-card {
    background: var(--bg-elevated);
    border: 1px solid var(--border);
    border-radius: 8px;
    padding: 12px;
    margin-bottom: 8px;
  }

  .volume-header {
    display: flex;
    align-items: center;
    gap: 8px;
    margin-bottom: 8px;
  }

  .drive-letter {
    font-size: 16px;
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

  .history-stats {
    display: flex;
    gap: 20px;
    margin-bottom: 16px;
  }

  .history-stat {
    text-align: center;
  }

  .history-stat .value {
    font-size: 18px;
    font-weight: 700;
    color: var(--teal);
    display: block;
  }

  .history-stat .label {
    font-size: 11px;
    color: var(--text-muted);
    display: block;
  }

  .sparkline {
    display: flex;
    gap: 4px;
    align-items: center;
    padding: 12px;
    background: var(--bg-deep);
    border-radius: 8px;
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

  .no-history {
    font-size: 12px;
    color: var(--text-muted);
    text-align: center;
    padding: 16px;
  }

  .offline-notice {
    padding: 8px 12px;
    background: color-mix(in srgb, var(--orange) 10%, transparent);
    border: 1px solid color-mix(in srgb, var(--orange) 30%, transparent);
    border-radius: 6px;
    color: var(--orange);
    font-size: 11px;
    margin-bottom: 8px;
  }

  .action-row {
    display: flex;
    gap: 8px;
    margin-top: 12px;
    flex-wrap: wrap;
  }

  .action-btn {
    padding: 6px 12px;
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
