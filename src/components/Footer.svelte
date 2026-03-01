<script lang="ts">
  import { app } from '../lib/stores/app.svelte';
</script>

<footer class="footer">
  <span class="footer-item">📋 {app.events.length} events</span>
  <span class="sep">·</span>
  <span class="footer-item">💾 {app.knownTotal} known</span>
  <span class="sep">·</span>
  <span class="footer-item online-count">🟢 {app.knownOnline} online</span>
  <span class="sep">·</span>
  <span class="footer-item log-path">📄 device-history.log</span>
  <span class="spacer"></span>
  <button class="footer-btn" onclick={() => app.exportEventsCSV()} title="Export events to CSV">
    Export CSV
  </button>
  <button class="footer-link" onclick={() => app.openUrl('https://tront.xyz')}>
    ⚡ tront.xyz
  </button>
</footer>

<style>
  .footer {
    display: flex;
    align-items: center;
    gap: 8px;
    padding: 6px 16px;
    background: linear-gradient(0deg, var(--bg-surface), color-mix(in srgb, var(--bg-surface) 97%, var(--accent)));
    border-top: 1px solid var(--border);
    font-size: 11px;
    color: var(--text-muted);
    position: relative;
  }
  .footer::before {
    content: '';
    position: absolute;
    top: 0;
    left: 0;
    right: 0;
    height: 1px;
    background: linear-gradient(90deg, transparent, var(--accent), transparent);
    opacity: 0.25;
  }
  .footer-item {
    white-space: nowrap;
  }
  .online-count {
    color: var(--green);
    font-weight: 500;
  }
  .log-path {
    font-family: 'Cascadia Code', 'Consolas', monospace;
    font-size: 10px;
    opacity: 0.7;
  }
  .sep {
    color: var(--accent);
    opacity: 0.4;
    font-size: 10px;
  }
  .spacer {
    flex: 1;
  }
  .footer-btn {
    background: none;
    border: 1px solid var(--border);
    border-radius: 6px;
    color: var(--text-muted);
    font-size: 11px;
    padding: 3px 10px;
    cursor: pointer;
    transition: all 150ms;
  }
  .footer-btn:hover {
    border-color: var(--accent);
    color: var(--accent);
  }
  .footer-link {
    font-size: 11px;
    color: var(--text-muted);
    padding: 2px 10px;
    border-radius: 8px;
    transition: all 150ms ease;
    font-weight: 500;
  }
  .footer-link:hover {
    color: var(--accent);
    background: var(--accent-glow);
    box-shadow: 0 0 8px var(--accent-glow);
  }
</style>
