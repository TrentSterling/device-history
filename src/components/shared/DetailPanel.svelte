<script lang="ts">
  import { app } from '../../lib/stores/app.svelte';
  import CapacityBar from './CapacityBar.svelte';

  let { deviceId, isConnected }: { deviceId: string; isConnected: boolean } = $props();

  let kd = $derived(app.knownDevices[deviceId] ?? null);
  let si = $derived(app.getStorageForDevice(deviceId));
</script>

<div class="detail-panel anim-slide-in">
  <!-- Storage info -->
  {#if si}
    <div class="section-label">💿 STORAGE</div>
    {#each si.volumes as vol}
      <div class="vol-header">
        <span class="vol-drive">💿 {vol.drive_letter}</span>
        {#if vol.volume_name}
          <span class="vol-name">"{vol.volume_name}"</span>
        {/if}
        <span class="vol-fs">({vol.file_system})</span>
      </div>
      <CapacityBar total={vol.total_bytes} free={vol.free_bytes} />
    {/each}

    <div class="info-row model-row">
      <span class="info-val">{si.model}</span>
      {#if si.serial_number}
        <span class="info-sec">🔑 {si.serial_number}</span>
      {/if}
    </div>

    {#if !isConnected}
      <div class="offline-notice">⚠️ OFFLINE — showing last known info</div>
    {/if}

    <div class="separator"></div>

    {#if si.interface_type}
      <div class="info-row"><span class="info-label">🔗 Interface:</span> <span class="info-val">{si.interface_type}</span></div>
    {/if}
    {#if si.firmware}
      <div class="info-row"><span class="info-label">⚙️ Firmware:</span> <span class="info-val">{si.firmware}</span></div>
    {/if}
    {#if si.status}
      <div class="info-row"><span class="info-label">📊 Status:</span> <span class="info-val">{si.status}</span></div>
    {/if}

    <div class="separator"></div>
  {:else if kd}
    <div class="dev-name">🔌 {kd.name}</div>
  {/if}

  <!-- Nickname editing -->
  <div class="section-label">✏️ NICKNAME</div>
  <div class="nickname-row">
    <input
      type="text"
      placeholder="e.g. My 4TB Seagate"
      bind:value={app.nicknameBuf}
    />
    <button class="save-btn" onclick={() => app.saveNickname()}>💾 Save</button>
  </div>

  <!-- Device info -->
  {#if kd}
    <div class="separator"></div>
    <div class="section-label">📋 DEVICE INFO</div>
    <div class="info-row"><span class="info-label">ID:</span> <span class="info-val mono">{kd.device_id}</span></div>
    <div class="info-row"><span class="info-label">VID:PID:</span> <span class="info-val">{kd.vid_pid || '—'}</span></div>
    <div class="info-row"><span class="info-label">Class:</span> <span class="info-val class-val">{kd.class}</span></div>
    <div class="info-row"><span class="info-label">Manufacturer:</span> <span class="info-val">{kd.manufacturer || '—'}</span></div>
    <div class="info-row"><span class="info-label">Description:</span> <span class="info-val">{kd.description || '—'}</span></div>

    <div class="separator"></div>
    <div class="section-label">📅 HISTORY</div>
    <div class="history-row">
      <span>📅 First: {kd.first_seen}</span>
      <span>🕐 Last: {kd.last_seen}</span>
      <span class="times-seen">🔄 {kd.times_seen}x</span>
    </div>
  {/if}

  <!-- Actions -->
  <div class="separator"></div>
  <div class="action-row">
    <button class="action-btn copy-btn" onclick={() => app.copyToClipboard(deviceId)}>📋 Copy ID</button>
    {#if si?.serial_number}
      <button class="action-btn copy-btn" onclick={() => app.copyToClipboard(si!.serial_number)}>🔑 Copy Serial</button>
    {/if}
    <button class="action-btn danger" onclick={() => app.forgetDevice(deviceId)}>🗑️ Forget</button>
  </div>
</div>

<style>
  .detail-panel {
    padding: 10px 12px;
    display: flex;
    flex-direction: column;
    gap: 4px;
  }
  .section-label {
    font-size: 11px;
    font-weight: 600;
    color: var(--cyan);
    letter-spacing: 0.5px;
    margin-top: 4px;
    padding-bottom: 2px;
  }
  .vol-header {
    display: flex;
    align-items: center;
    gap: 8px;
  }
  .vol-drive {
    font-size: 18px;
    color: var(--green);
    font-weight: 600;
  }
  .vol-name {
    font-size: 15px;
    color: var(--text);
    font-weight: 500;
  }
  .vol-fs {
    font-size: 11px;
    color: var(--text-sec);
  }
  .dev-name {
    font-size: 14px;
    color: var(--text);
    font-weight: 500;
    padding: 4px 0;
  }
  .offline-notice {
    font-size: 10px;
    color: var(--orange);
    margin-top: 2px;
    padding: 3px 8px;
    background: color-mix(in srgb, var(--orange) 8%, transparent);
    border-radius: 4px;
    border: 1px solid color-mix(in srgb, var(--orange) 20%, transparent);
  }
  .model-row {
    margin-top: 2px;
  }
  .info-row {
    display: flex;
    gap: 6px;
    font-size: 11px;
    line-height: 1.6;
  }
  .info-label {
    color: var(--text-sec);
    flex-shrink: 0;
  }
  .info-val {
    color: var(--text);
  }
  .info-sec {
    color: var(--text-sec);
    font-size: 10px;
  }
  .class-val {
    color: var(--accent);
  }
  .mono {
    font-family: 'Cascadia Code', 'Consolas', monospace;
    font-size: 10px;
    word-break: break-all;
    color: var(--text-muted);
  }
  .history-row {
    display: flex;
    gap: 14px;
    font-size: 11px;
    color: var(--text-sec);
    flex-wrap: wrap;
  }
  .times-seen {
    color: var(--teal);
    font-weight: 600;
  }
  .separator {
    height: 1px;
    background: linear-gradient(90deg, transparent, var(--border), transparent);
    margin: 4px 0;
  }
  .nickname-row {
    display: flex;
    align-items: center;
    gap: 6px;
    margin: 2px 0;
  }
  .nickname-row input {
    flex: 1;
    max-width: 240px;
    font-size: 12px;
    padding: 4px 8px;
    background: var(--bg-deep);
    border: 1px solid var(--border);
    border-radius: 6px;
    color: var(--text);
    outline: none;
    transition: border-color 200ms ease, box-shadow 200ms ease;
  }
  .nickname-row input:focus {
    border-color: var(--teal);
    box-shadow: 0 0 8px color-mix(in srgb, var(--teal) 20%, transparent);
  }
  .nickname-row input::placeholder {
    color: var(--text-muted);
    font-size: 11px;
  }
  .save-btn {
    padding: 4px 10px;
    font-size: 11px;
    font-weight: 500;
    color: var(--teal);
    border: 1px solid var(--teal);
    border-radius: 6px;
    background: transparent;
    cursor: pointer;
    transition: all 180ms ease;
  }
  .save-btn:hover {
    background: color-mix(in srgb, var(--teal) 12%, transparent);
    box-shadow: 0 0 8px color-mix(in srgb, var(--teal) 20%, transparent);
  }
  .action-row {
    display: flex;
    gap: 6px;
    flex-wrap: wrap;
  }
  .action-btn {
    padding: 4px 10px;
    font-size: 11px;
    font-weight: 500;
    border-radius: 6px;
    cursor: pointer;
    transition: all 180ms ease;
  }
  .copy-btn {
    color: var(--text-sec);
    border: 1px solid var(--border);
    background: transparent;
  }
  .copy-btn:hover {
    border-color: var(--accent);
    color: var(--accent);
    background: color-mix(in srgb, var(--accent) 8%, transparent);
    box-shadow: 0 0 6px color-mix(in srgb, var(--accent) 15%, transparent);
  }
  .action-btn.danger {
    color: var(--red);
    border: 1px solid color-mix(in srgb, var(--red) 40%, transparent);
    background: transparent;
  }
  .action-btn.danger:hover {
    border-color: var(--red);
    background: color-mix(in srgb, var(--red) 10%, transparent);
    box-shadow: 0 0 6px color-mix(in srgb, var(--red) 20%, transparent);
  }
</style>
