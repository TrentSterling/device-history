<script lang="ts">
  import { app } from '../lib/stores/app.svelte';
  import { themes } from '../lib/themes';
</script>

<header class="header">
  <div class="header-left">
    <span class="logo">🔌</span>
    <span class="title">Device History</span>
    <span class="version">v0.8.0</span>
    {#if app.updateAvailable}
      <button class="update-badge" onclick={() => app.openUrl('https://github.com/TrentSterling/device-history/releases/latest')}>
        🆕 v{app.updateAvailable}
      </button>
    {/if}
  </div>
  <div class="header-right">
    <div class="theme-switcher">
      {#each themes as t}
        <button
          class="pill"
          class:active={app.theme === t.id}
          onclick={() => app.setTheme(t.id)}
        >
          {t.label}
        </button>
      {/each}
    </div>
    <div class="status-pill">
      {#if app.error}
        <span class="error-icon">⚠️</span>
        <span class="error-text">{app.error}</span>
      {:else}
        <span class="status-dot online"></span>
        <span class="monitoring-text">Monitoring {app.devices.length} devices</span>
      {/if}
    </div>
  </div>
</header>

<style>
  .header {
    display: flex;
    align-items: center;
    justify-content: space-between;
    padding: 10px 16px;
    background: linear-gradient(180deg,
      color-mix(in srgb, var(--bg-surface) 95%, var(--accent)),
      var(--bg-surface));
    border-bottom: 1px solid var(--border);
    box-shadow: 0 2px 12px var(--shadow-color);
    gap: 12px;
    position: relative;
    z-index: 10;
  }
  .header::after {
    content: '';
    position: absolute;
    bottom: 0;
    left: 0;
    right: 0;
    height: 1px;
    background: linear-gradient(90deg, transparent, var(--accent), transparent);
    opacity: 0.3;
  }
  .header-left {
    display: flex;
    align-items: center;
    gap: 8px;
  }
  .header-right {
    display: flex;
    align-items: center;
    gap: 14px;
  }
  .logo {
    font-size: 24px;
    filter: drop-shadow(0 0 6px var(--accent-glow));
    animation: float 4s ease-in-out infinite;
    transition: transform 300ms cubic-bezier(0.34, 1.56, 0.64, 1);
  }
  .header-left:hover .logo {
    transform: translateX(-2px) translateY(-1px) scale(1.1);
  }
  .title {
    font-size: 20px;
    font-weight: 700;
    background: linear-gradient(135deg, var(--accent), var(--pink));
    -webkit-background-clip: text;
    -webkit-text-fill-color: transparent;
    background-clip: text;
    text-shadow: none;
    transition: transform 300ms cubic-bezier(0.34, 1.56, 0.64, 1);
  }
  .header-left:hover .title {
    transform: translateX(2px);
  }
  .version {
    font-size: 11px;
    color: var(--accent);
    padding: 2px 8px;
    background: var(--accent-glow);
    border-radius: 10px;
    border: 1px solid color-mix(in srgb, var(--accent) 30%, var(--border));
    font-weight: 600;
    letter-spacing: 0.3px;
  }
  .update-badge {
    padding: 3px 10px;
    font-size: 11px;
    font-weight: 600;
    color: var(--orange);
    border: 1px solid var(--orange);
    border-radius: 6px;
    background: linear-gradient(135deg, transparent, color-mix(in srgb, var(--orange) 8%, transparent));
    animation: float 3s ease-in-out infinite;
  }
  .update-badge:hover {
    background: color-mix(in srgb, var(--bg-surface) 80%, var(--orange));
    box-shadow: 0 0 12px color-mix(in srgb, var(--orange) 30%, transparent);
  }
  .theme-switcher {
    display: flex;
    gap: 4px;
  }
  .status-pill {
    display: flex;
    align-items: center;
    gap: 6px;
    font-size: 13px;
    padding: 4px 12px;
    border-radius: 20px;
    background: var(--glass-bg);
    backdrop-filter: blur(8px);
    border: 1px solid var(--glass-border);
  }
  .monitoring-text {
    color: var(--teal);
    font-weight: 500;
  }
  .error-text {
    color: var(--pink);
  }
</style>
