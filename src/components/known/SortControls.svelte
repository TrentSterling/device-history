<script lang="ts">
  import { app, type SortMode } from '../../lib/stores/app.svelte';

  const modes: { id: SortMode; label: string; icon: string }[] = [
    { id: 'status', label: 'Status', icon: '🟢' },
    { id: 'name', label: 'Name', icon: '🔤' },
    { id: 'last_seen', label: 'Last', icon: '🕐' },
    { id: 'times_seen', label: 'Count', icon: '🔄' },
    { id: 'first_seen', label: 'First', icon: '📅' },
  ];
</script>

<div class="sort-controls">
  {#each modes as mode}
    {@const isActive = app.sortMode === mode.id}
    {@const arrow = isActive ? (app.sortAscending ? ' ▲' : ' ▼') : ''}
    <button
      class="sort-pill"
      class:active={isActive}
      onclick={() => app.toggleSort(mode.id)}
    >
      {mode.icon} {mode.label}{arrow}
    </button>
  {/each}
</div>

<style>
  .sort-controls {
    display: flex;
    gap: 4px;
    flex-wrap: wrap;
  }
  .sort-pill {
    padding: 3px 8px;
    font-size: 10px;
    font-weight: 500;
    color: var(--text-sec);
    background: color-mix(in srgb, var(--bg-surface) 80%, transparent);
    border: 1px solid var(--border);
    border-radius: 12px;
    cursor: pointer;
    transition: all 180ms ease;
    white-space: nowrap;
  }
  .sort-pill:hover {
    border-color: var(--accent);
    color: var(--text);
    background: color-mix(in srgb, var(--accent) 8%, var(--bg-surface));
  }
  .sort-pill.active {
    color: var(--accent);
    border-color: var(--accent);
    background: var(--accent-glow, color-mix(in srgb, var(--accent) 12%, transparent));
    box-shadow: 0 0 8px color-mix(in srgb, var(--accent) 15%, transparent);
    font-weight: 600;
  }
</style>
