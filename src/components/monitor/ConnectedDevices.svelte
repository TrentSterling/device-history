<script lang="ts">
  import { app } from '../../lib/stores/app.svelte';
  import ConnectedDeviceCard from './ConnectedDeviceCard.svelte';
</script>

<div class="connected-devices glass-panel">
  {#if app.devices.length === 0}
    <div class="empty">
      <span class="empty-icon">🔍</span>
      <span>No devices connected</span>
    </div>
  {:else}
    <div class="device-list">
      {#each app.devices as device (device.device_id)}
        <ConnectedDeviceCard {device} />
      {/each}
    </div>
  {/if}
</div>

<style>
  .connected-devices {
    flex: 1;
    min-height: 0;
    overflow-y: auto;
    padding: 8px;
  }
  .device-list {
    display: flex;
    flex-direction: column;
    gap: 3px;
  }
  .empty {
    padding: 24px;
    text-align: center;
    color: var(--text-sec);
    font-size: 13px;
    display: flex;
    flex-direction: column;
    align-items: center;
    gap: 8px;
  }
  .empty-icon {
    font-size: 28px;
    opacity: 0.7;
  }
</style>
