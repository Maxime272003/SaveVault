<script lang="ts">
  import Backups from "$lib/components/Backups.svelte";
  import FirstRun from "$lib/components/FirstRun.svelte";
  import Library from "$lib/components/Library.svelte";
  import Settings from "$lib/components/Settings.svelte";
  import Sidebar from "$lib/components/Sidebar.svelte";
  import Toasts from "$lib/components/Toasts.svelte";
  import Unidentified from "$lib/components/Unidentified.svelte";
  import { app } from "$lib/store.svelte";

  let ready = $state(false);
  let firstRunDismissed = $state(false);

  $effect(() => {
    app
      .init()
      .catch((e) => app.fail(e))
      .finally(() => (ready = true));
  });
</script>

<div class="shell">
  <Sidebar />
  {#if app.view === "library"}
    <Library />
  {:else if app.view === "unidentified"}
    <Unidentified />
  {:else if app.view === "backups"}
    <Backups />
  {:else}
    <Settings />
  {/if}
</div>

{#if ready && app.config && !app.config.backupDir && !firstRunDismissed}
  <FirstRun onclose={() => (firstRunDismissed = true)} />
{/if}

<Toasts />

<style>
  .shell {
    height: 100vh;
    display: flex;
    overflow: hidden;
  }
</style>
