<script lang="ts">
  import { app } from '../lib/stores/app.svelte';
</script>

{#if app.notifications.length > 0}
  <div class="toast-container">
    {#each app.notifications as notif (notif.id)}
      <div class="toast {notif.kind} anim-slide-in">
        <span class="toast-icon">
          {#if notif.kind === 'success'}✅
          {:else if notif.kind === 'error'}❌
          {:else}ℹ️
          {/if}
        </span>
        <span class="toast-text">{notif.text}</span>
      </div>
    {/each}
  </div>
{/if}

<style>
  .toast-container {
    position: fixed;
    bottom: 16px;
    right: 16px;
    z-index: 1000;
    display: flex;
    flex-direction: column;
    gap: 6px;
    pointer-events: none;
  }
  .toast {
    display: flex;
    align-items: center;
    gap: 8px;
    padding: 8px 14px;
    border-radius: 10px;
    font-size: 12px;
    font-weight: 500;
    color: var(--text);
    background: var(--glass-bg, color-mix(in srgb, var(--bg-elevated) 85%, transparent));
    backdrop-filter: blur(16px);
    -webkit-backdrop-filter: blur(16px);
    border: 1px solid var(--glass-border, var(--border));
    box-shadow: 0 4px 20px color-mix(in srgb, var(--shadow-color, black) 30%, transparent);
    pointer-events: auto;
  }
  .toast.success {
    border-color: color-mix(in srgb, var(--green) 30%, var(--border));
    box-shadow: 0 4px 20px color-mix(in srgb, var(--green) 10%, transparent);
  }
  .toast.error {
    border-color: color-mix(in srgb, var(--red) 30%, var(--border));
    box-shadow: 0 4px 20px color-mix(in srgb, var(--red) 10%, transparent);
  }
  .toast-icon {
    font-size: 14px;
    flex-shrink: 0;
  }
  .toast-text {
    flex: 1;
  }
</style>
