<script lang="ts">
  import { app } from './lib/stores/app.svelte';
  import { themeClass } from './lib/themes';
  import Header from './components/Header.svelte';
  import TabBar from './components/TabBar.svelte';
  import Footer from './components/Footer.svelte';
  import Notification from './components/Notification.svelte';
  import MonitorTab from './components/monitor/MonitorTab.svelte';
  import KnownDevicesTab from './components/known/KnownDevicesTab.svelte';
</script>

<div class={themeClass(app.theme)} id="app-root">
  <Header />
  <TabBar />
  <main class="content">
    {#if app.activeTab === 'monitor'}
      <MonitorTab />
    {:else}
      <KnownDevicesTab />
    {/if}
  </main>
  <Footer />
  <Notification />
</div>

<style>
  #app-root {
    display: flex;
    flex-direction: column;
    height: 100vh;
    background: var(--bg-deep);
    color: var(--text);
  }
  .content {
    flex: 1;
    overflow: hidden;
    display: flex;
    flex-direction: column;
  }
</style>
