<script lang="ts">
  import { app } from '../lib/stores/app.svelte';

  const tabs = [
    { id: 'monitor' as const, label: '📡 Monitor', icon: '' },
    { id: 'known' as const, label: '💾 Known Devices', icon: '' },
  ];
</script>

<div class="tab-bar">
  <div class="tabs">
    {#each tabs as tab}
      <button
        class="tab-btn"
        class:active={app.activeTab === tab.id}
        onclick={() => app.setActiveTab(tab.id)}
      >
        {tab.label}
      </button>
    {/each}
  </div>
  <div class="tab-stats">
    <span class="stat-number">{app.knownTotal}</span> known
    <span class="stat-dot">·</span>
    <span class="stat-online">{app.knownOnline}</span> online
  </div>
</div>

<style>
  .tab-bar {
    display: flex;
    align-items: center;
    justify-content: space-between;
    padding: 8px 16px 0;
    background: linear-gradient(180deg, var(--bg-surface), color-mix(in srgb, var(--bg-surface) 97%, var(--bg-deep)));
    border-bottom: 1px solid var(--border);
  }
  .tabs {
    display: flex;
    gap: 6px;
  }
  .tab-btn {
    padding: 8px 18px;
    font-size: 13px;
    font-weight: 500;
    border-radius: 8px 8px 0 0;
    color: var(--text-sec);
    border: 1px solid var(--border);
    border-bottom: none;
    background: linear-gradient(180deg, var(--bg-elevated), var(--bg-surface));
    transition: all 200ms ease;
    position: relative;
  }
  .tab-btn:hover {
    color: var(--text);
    background: linear-gradient(180deg,
      color-mix(in srgb, var(--bg-elevated) 85%, var(--accent)),
      var(--bg-surface));
    border-color: color-mix(in srgb, var(--border) 60%, var(--accent));
  }
  .tab-btn.active {
    background: linear-gradient(180deg, var(--bg-deep), var(--bg-deep));
    color: var(--accent);
    border-color: var(--accent);
    box-shadow: 0 -2px 12px var(--accent-glow);
    text-shadow: 0 0 8px var(--accent-glow-strong);
  }
  .tab-btn.active::before {
    content: '';
    position: absolute;
    top: 0;
    left: 20%;
    right: 20%;
    height: 2px;
    background: linear-gradient(90deg, transparent, var(--accent), transparent);
    border-radius: 0 0 2px 2px;
  }
  .tab-btn.active::after {
    content: '';
    position: absolute;
    bottom: -1px;
    left: 0;
    right: 0;
    height: 2px;
    background: var(--bg-deep);
  }
  .tab-stats {
    font-size: 12px;
    color: var(--text-muted);
    padding: 4px 12px;
    border-radius: 12px;
    background: var(--bg-deep);
    border: 1px solid var(--border);
  }
  .stat-number {
    color: var(--text-sec);
    font-weight: 600;
  }
  .stat-dot {
    color: var(--accent);
    margin: 0 2px;
  }
  .stat-online {
    color: var(--green);
    font-weight: 600;
  }
</style>
