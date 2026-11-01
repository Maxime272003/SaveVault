<script lang="ts">
  import { api, type BackupInfo } from "$lib/api";
  import { formatDate, formatSize } from "$lib/format";
  import { t } from "$lib/i18n.svelte";
  import { app } from "$lib/store.svelte";
  import Dialog from "./Dialog.svelte";
  import Icon from "./Icon.svelte";

  let filter = $state("");
  let restoring = $state<BackupInfo | null>(null);
  let deleting = $state<BackupInfo | null>(null);

  const list = $derived(
    app.backups.filter((b) => !filter.trim() || b.game.toLowerCase().includes(filter.trim().toLowerCase())),
  );
  const total = $derived(app.backups.reduce((s, b) => s + b.size, 0));

  async function restore(b: BackupInfo) {
    try {
      await api.restoreBackup(b.path);
      app.toast(t("backup.restored"));
      await app.refreshBackups();
      await app.rescan();
    } catch (e) {
      app.fail(e);
    }
  }

  async function remove(b: BackupInfo) {
    try {
      await api.deleteBackup(b.path);
      app.toast(t("backup.deleted"));
      await app.refreshBackups();
    } catch (e) {
      app.fail(e);
    }
  }

  const isCloud = (b: BackupInfo) => app.games.find((g) => g.title === b.game)?.cloudSteam ?? false;
</script>

<div class="page">
  <div class="head">
    <div>
      <h1>{t("backups.title")}</h1>
      <span class="muted">{t("backups.summary", { n: app.backups.length, size: formatSize(total) })}</span>
    </div>
    <div class="tools">
      <input class="input" placeholder={t("backups.filter")} aria-label={t("backups.filter")} bind:value={filter} />
      {#if app.config?.backupDir}
        <button class="btn" onclick={() => api.openPath(app.config!.backupDir!).catch((e) => app.fail(e))}>
          <Icon name="folder" />
          {t("backups.openDir")}
        </button>
      {/if}
    </div>
  </div>

  {#if !app.config?.backupDir}
    <p class="muted">{t("backups.noDir")}</p>
  {:else if list.length === 0}
    <p class="muted">{t("backups.empty")}</p>
  {:else}
    <div class="table" role="table">
      {#each list as b (b.path)}
        <div class="row" role="row">
          <div class="game" role="cell">
            <span class="name">{b.game}</span>
            <span class="muted small">{b.fileName}</span>
          </div>
          <div role="cell">{formatDate(b.createdAt)}</div>
          <div role="cell">
            {#if b.kind !== "manual"}<span class="chip">{t(`backup.kind.${b.kind}`)}</span>{/if}
          </div>
          <div role="cell" class="size">{formatSize(b.size)}</div>
          <div role="cell" class="actions">
            <button class="btn small restore" onclick={() => (restoring = b)}>{t("backup.restore")}</button>
            <button class="btn small" onclick={() => api.openPath(b.path).catch((e) => app.fail(e))}>{t("backup.reveal")}</button>
            <button class="btn small danger" aria-label={t("backup.delete")} title={t("backup.delete")} onclick={() => (deleting = b)}>
              <Icon name="trash" size={14} />
            </button>
          </div>
        </div>
      {/each}
    </div>
  {/if}
</div>

{#if restoring}
  {@const b = restoring}
  <Dialog
    title={t("confirm.restoreTitle", { game: b.game })}
    icon="download"
    tone="accent"
    confirmLabel={t("confirm.restoreAction")}
    onconfirm={() => restore(b)}
    onclose={() => (restoring = null)}
  >
    <span>{formatDate(b.createdAt)} · {formatSize(b.size)}</span>
    <code class="mono">{b.fileName}</code>
    <span>{t("confirm.restoreInfo")}</span>
    {#if isCloud(b)}<div class="callout info">{t("game.cloudWarning")}</div>{/if}
  </Dialog>
{/if}

{#if deleting}
  {@const b = deleting}
  <Dialog
    title={t("confirm.deleteBackupTitle")}
    icon="trash"
    tone="danger"
    confirmLabel={t("backup.delete")}
    onconfirm={() => remove(b)}
    onclose={() => (deleting = null)}
  >
    <code class="mono">{b.path}</code>
    <span>{t("confirm.deleteBackupInfo")}</span>
  </Dialog>
{/if}

<style>
  .page {
    flex: 1;
    min-width: 0;
    overflow-y: auto;
    padding: 32px 40px;
  }

  .head {
    display: flex;
    justify-content: space-between;
    align-items: flex-end;
    gap: 16px;
    flex-wrap: wrap;
    margin-bottom: 24px;
  }

  h1 {
    margin: 0 0 4px;
    font-family: var(--font-display);
    font-size: 28px;
  }

  .tools {
    display: flex;
    gap: 8px;
  }

  .table {
    display: flex;
    flex-direction: column;
  }

  .row {
    display: grid;
    grid-template-columns: minmax(0, 2fr) 180px 160px 90px auto;
    gap: 16px;
    align-items: center;
    padding: 12px 4px;
    border-bottom: 1px solid var(--divider);
  }

  .game {
    display: flex;
    flex-direction: column;
    gap: 2px;
    min-width: 0;
  }

  .game span {
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }

  .name {
    font-weight: 600;
  }

  .small {
    font-size: 12px;
  }

  .size {
    text-align: right;
    color: var(--muted);
  }

  .actions {
    display: flex;
    gap: 6px;
    justify-content: flex-end;
  }

  .restore {
    background: transparent;
    color: var(--accent);
    border-color: var(--accent-border);
  }
</style>
