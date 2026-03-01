<script lang="ts">
  import { app } from '../../lib/stores/app.svelte';
  import AboutBanner from './AboutBanner.svelte';
  import EventLog from './EventLog.svelte';
  import ConnectedDevices from './ConnectedDevices.svelte';
</script>

<div class="monitor-tab">
  <AboutBanner />

  <div class="section-header">
    <span class="section-title">📜 Event Log ({app.events.length})</span>
    <button class="clear-btn" onclick={() => app.clearEvents()}>
      🗑️ Clear
    </button>
  </div>

  <EventLog />

  <div class="rainbow-separator"></div>

  <div class="section-header">
    <span class="section-title">🔗 Connected ({app.devices.length})</span>
  </div>

  <ConnectedDevices />
</div>

<style>
  .monitor-tab {
    display: flex;
    flex-direction: column;
    gap: 8px;
    padding: 14px 16px;
    flex: 1;
    overflow: hidden;
  }
  .section-header {
    display: flex;
    align-items: center;
    justify-content: space-between;
  }
  .section-title {
    font-size: 14px;
    font-weight: 600;
    background: linear-gradient(135deg, var(--cyan), var(--teal));
    -webkit-background-clip: text;
    -webkit-text-fill-color: transparent;
    background-clip: text;
  }
  .clear-btn {
    padding: 3px 10px;
    font-size: 11px;
    font-weight: 500;
    color: var(--orange);
    border: 1px solid var(--orange);
    border-radius: 6px;
    background: transparent;
    transition: all 180ms ease;
  }
  .clear-btn:hover {
    background: color-mix(in srgb, var(--bg-surface) 80%, var(--orange));
    box-shadow: 0 0 8px color-mix(in srgb, var(--orange) 25%, transparent);
  }
</style>
