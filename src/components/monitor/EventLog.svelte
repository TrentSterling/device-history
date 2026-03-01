<script lang="ts">
  import { app } from '../../lib/stores/app.svelte';
  import EventCard from './EventCard.svelte';
</script>

<div class="event-log glass-panel">
  {#if app.events.length === 0}
    <div class="empty">
      <span class="empty-icon">👀</span>
      <span class="empty-text">No events yet — waiting for USB changes...</span>
      <span class="empty-hint">Plug or unplug a device to see it here</span>
    </div>
  {:else}
    <div class="event-list">
      {#each app.events as event, i (event.timestamp + event.device_id + i)}
        <EventCard {event} />
      {/each}
    </div>
  {/if}
</div>

<style>
  .event-log {
    flex: 1;
    min-height: 0;
    overflow-y: auto;
    padding: 8px;
  }
  .event-list {
    display: flex;
    flex-direction: column;
    gap: 4px;
  }
  .empty {
    padding: 32px 24px;
    text-align: center;
    color: var(--text-sec);
    font-size: 13px;
    display: flex;
    flex-direction: column;
    align-items: center;
    gap: 6px;
  }
  .empty-icon {
    font-size: 36px;
    filter: drop-shadow(0 0 8px var(--accent-glow));
  }
  .empty-text {
    font-weight: 500;
  }
  .empty-hint {
    font-size: 11px;
    color: var(--text-muted);
    font-style: italic;
  }
</style>
