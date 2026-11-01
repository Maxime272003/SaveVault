<script lang="ts">
  import { documentDir, join } from "@tauri-apps/api/path";
  import { open } from "@tauri-apps/plugin-dialog";
  import { t } from "$lib/i18n.svelte";
  import { app } from "$lib/store.svelte";
  import Dialog from "./Dialog.svelte";

  let { onclose }: { onclose: () => void } = $props();
  let dir = $state("");

  $effect(() => {
    documentDir()
      .then((docs) => join(docs, "SaveVault", "Backups"))
      .then((d) => (dir = d));
  });

  async function choose() {
    const picked = await open({ directory: true });
    if (typeof picked === "string") dir = picked;
  }
</script>

<Dialog
  title={t("first.title")}
  icon="folder"
  tone="accent"
  confirmLabel={t("first.use")}
  confirmDisabled={!dir}
  onconfirm={async () => {
    await app.saveConfig({ backupDir: dir });
    await app.refreshBackups();
  }}
  {onclose}
>
  <p>{t("first.body")}</p>
  <code class="mono path">{dir}</code>
  <div>
    <button class="btn" onclick={choose}>{t("first.choose")}</button>
  </div>
</Dialog>

<style>
  p {
    margin: 0;
  }

  .path {
    background: var(--surface);
    border: 1px solid #262b35;
    border-radius: var(--radius);
    padding: 10px 12px;
  }
</style>
