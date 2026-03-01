<script lang="ts">
  import { app } from "../../lib/stores/app.svelte";
  import { CLASS_FILTERS } from "../../lib/utils";
</script>

<div class="class-filter">
  {#each CLASS_FILTERS as filter}
    <button
      class="filter-pill"
      class:active={app.classFilter === filter}
      onclick={() => app.setClassFilter(filter)}
    >
      {filter}
    </button>
  {/each}
</div>

<style>
  .class-filter {
    display: flex;
    gap: 4px;
    flex-wrap: wrap;
  }
  .filter-pill {
    padding: 4px 12px;
    border-radius: 14px;
    border: 1px solid var(--border);
    background: var(--bg-surface);
    color: var(--text-muted);
    font-size: 11px;
    font-weight: 500;
    cursor: pointer;
    transition: all 150ms ease;
  }
  .filter-pill:hover {
    border-color: var(--accent);
    color: var(--text);
  }
  .filter-pill.active {
    background: var(--accent-glow);
    border-color: var(--accent);
    color: var(--accent);
  }
</style>
