<script lang="ts">
  import { app } from '../../lib/stores/app.svelte';
</script>

<div class="search-bar">
  <span class="search-icon">🔍</span>
  <input
    type="text"
    placeholder="Search by name, class, manufacturer, VID:PID..."
    bind:value={app.searchQuery}
  />
  {#if app.searchQuery}
    <button class="clear-btn" onclick={() => app.searchQuery = ''}>✕</button>
  {/if}
</div>

<style>
  .search-bar {
    display: flex;
    align-items: center;
    gap: 6px;
    background: var(--glass-bg, color-mix(in srgb, var(--bg-surface) 70%, transparent));
    border: 1px solid var(--glass-border, var(--border));
    border-radius: 8px;
    padding: 2px 10px;
    backdrop-filter: blur(8px);
    -webkit-backdrop-filter: blur(8px);
    transition: border-color 200ms ease, box-shadow 200ms ease;
  }
  .search-bar:focus-within {
    border-color: var(--accent);
    box-shadow: 0 0 12px color-mix(in srgb, var(--accent) 20%, transparent);
  }
  .search-icon {
    font-size: 13px;
    flex-shrink: 0;
  }
  input {
    width: 280px;
    background: transparent;
    border: none;
    outline: none;
    color: var(--text);
    font-size: 12px;
    padding: 4px 0;
  }
  input::placeholder {
    color: var(--text-muted);
    font-size: 11px;
  }
  .clear-btn {
    font-size: 12px;
    color: var(--text-sec);
    padding: 2px 6px;
    border-radius: 4px;
    background: transparent;
    border: none;
    cursor: pointer;
    transition: all 150ms ease;
  }
  .clear-btn:hover {
    background: color-mix(in srgb, var(--red) 15%, transparent);
    color: var(--red);
  }
</style>
