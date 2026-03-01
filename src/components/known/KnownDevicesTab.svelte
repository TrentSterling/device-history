<script lang="ts">
  import { app } from '../../lib/stores/app.svelte';
  import SearchBar from './SearchBar.svelte';
  import SortControls from './SortControls.svelte';
  import KnownDeviceCard from './KnownDeviceCard.svelte';
  import ClassFilter from '../shared/ClassFilter.svelte';
</script>

<div class="known-tab">
  <div class="controls-row">
    <SearchBar />
    <SortControls />
  </div>
  <ClassFilter />

  <div class="device-list-container glass-panel scroll-shadow">
    {#if app.isLoading}
      <div style="display: flex; flex-direction: column; gap: 6px; padding: 8px;">
        <div class="skeleton-card"></div>
        <div class="skeleton-card"></div>
        <div class="skeleton-card"></div>
      </div>
    {:else if Object.keys(app.knownDevices).length === 0}
      <div class="empty">
        <span class="empty-icon">🔌</span>
        <span>No devices seen yet — plug in a USB device to get started</span>
      </div>
    {:else if app.filteredKnown.length === 0}
      <div class="empty">
        <span class="empty-icon">🔍</span>
        <span>No devices matching '<strong>{app.searchQuery}</strong>'</span>
      </div>
    {:else}
      <div class="list-header">
        <span class="count-badge">💾 {app.filteredKnown.length} device{app.filteredKnown.length !== 1 ? 's' : ''}</span>
      </div>
      <div class="device-list">
        {#each app.filteredKnown as device (device.device_id)}
          <KnownDeviceCard {device} />
        {/each}
      </div>
    {/if}
  </div>
</div>

<style>
  .known-tab {
    display: flex;
    flex-direction: column;
    gap: 8px;
    padding: 14px 16px;
    flex: 1;
    overflow: hidden;
  }
  .controls-row {
    display: flex;
    align-items: center;
    gap: 12px;
    flex-wrap: wrap;
  }
  .device-list-container {
    flex: 1;
    min-height: 0;
    overflow-y: auto;
    padding: 8px;
  }
  .list-header {
    display: flex;
    align-items: center;
    padding: 0 4px 6px;
  }
  .count-badge {
    font-size: 11px;
    color: var(--text-sec);
    font-weight: 500;
  }
  .device-list {
    display: flex;
    flex-direction: column;
    gap: 3px;
  }
  .empty {
    padding: 32px 24px;
    text-align: center;
    color: var(--text-sec);
    font-size: 13px;
    display: flex;
    flex-direction: column;
    align-items: center;
    gap: 8px;
  }
  .empty-icon {
    font-size: 32px;
    opacity: 0.7;
  }
</style>
