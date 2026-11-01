<script lang="ts">
  import { api, type BackupInfo } from "$lib/api";
  import { formatDate, formatSize, relativeTime } from "$lib/format";
  import { t } from "$lib/i18n.svelte";
  import { app } from "$lib/store.svelte";
  import Cover from "./Cover.svelte";
  import Dialog from "./Dialog.svelte";
  import Icon from "./Icon.svelte";

  const game = $derived(app.selected);
  const backups = $derived(game ? app.backupsFor(game.title) : []);

  let running = $state<boolean | null>(null);
  let busy = $state(false);
  let dialog = $state<"delete" | "restore" | null>(null);
  let restoreChoice = $state<string | null>(null);

  $effect(() => {
    const id = game?.id;
    running = null;
    if (!id) return;
    let cancelled = false;
    api.isGameRunning(id).then((r) => {
      if (!cancelled) running = r;
    });
    return () => {
      cancelled = true;
    };
  });

  function needBackupDir(): boolean {
    if (app.config?.backupDir) return false;
    app.view = "settings";
    app.toast(t("err.no_backup_dir"), "error");
    return true;
  }

  async function backup() {
    if (!game || needBackupDir()) return;
    busy = true;
    try {
      const info = await api.createBackup(game.id);
      app.toast(t("backup.created", { name: info.fileName }));
      await app.refreshBackups();
    } catch (e) {
      app.fail(e);
    } finally {
      busy = false;
    }
  }

  function openRestore() {
    if (needBackupDir()) return;
    restoreChoice = backups[0]?.path ?? null;
    dialog = "restore";
  }

  async function restore() {
    if (!restoreChoice) return;
    try {
      await api.restoreBackup(restoreChoice);
      app.toast(t("backup.restored"));
      await app.refreshBackups();
      await app.rescan();
    } catch (e) {
      app.fail(e);
    }
  }

  async function restoreOne(b: BackupInfo) {
    if (needBackupDir()) return;
    restoreChoice = b.path;
    dialog = "restore";
  }

  async function remove() {
    if (!game) return;
    const title = game.title;
    try {
      await api.deleteGameSaves(game.id);
      app.toast(t("confirm.done", { game: title }));
      await app.refreshBackups();
      app.scan = await api.getCachedScan();
      app.selectedId = app.filtered[0]?.id ?? null;
    } catch (e) {
      app.fail(e);
    }
  }

  function openDelete() {
    if (needBackupDir()) return;
    dialog = "delete";
  }

  const kindLabel = (b: BackupInfo) => (b.kind === "manual" ? "" : ` · ${t(`backup.kind.${b.kind}`)}`);
</script>

<aside>
  {#if game}
    <div class="hero">
      <Cover appId={game.steamAppId} title={game.title} showTitle={false} position="center 20%" />
      <div class="shade"></div>
      <h1>{game.title}</h1>
    </div>

    <div class="content">
      <div class="chips">
        <span class="chip">{t(`source.${game.source}`)}</span>
        <span class="chip">{game.installed ? t("filter.installed") : t("filter.notInstalled")}</span>
        {#if game.cloudSteam}<span class="chip cloud">Steam Cloud</span>{/if}
        {#each game.accounts as acc (acc)}
          <span class="chip">{t("game.account", { name: acc })}</span>
        {/each}
      </div>

      {#if running}
        <div class="callout danger">{t("game.running")}</div>
      {/if}

      <section>
        <span class="caps">
          {game.locations.length > 1 ? t("game.locations", { n: game.locations.length }) : t("game.location")}
        </span>
        {#each game.locations as loc (loc.path)}
          <div class="loc">
            <div class="loc-head">
              <code class="mono">{loc.path}</code>
              <button class="icon-btn" aria-label={t("game.openLocation")} title={t("game.openLocation")} onclick={() => api.openPath(loc.path).catch((e) => app.fail(e))}>
                <Icon name="folder" />
              </button>
            </div>
            <div class="loc-meta">
              <span>
                {formatSize(loc.size)} · {t("game.files", { n: loc.fileCount })}
                {#if loc.custom}· {t("game.custom")}{/if}
              </span>
              <span>{t("game.modified", { when: relativeTime(loc.modified) })}</span>
            </div>
          </div>
        {/each}
      </section>

      <div class="actions">
        <button class="btn tall primary wide" onclick={backup} disabled={busy}>
          <Icon name="download" />
          {t("game.backup")}
        </button>
        <button class="btn tall" onclick={() => api.openPath(game.locations[0].path).catch((e) => app.fail(e))}>
          {t("game.open")}
        </button>
        <button class="btn tall" onclick={openRestore} disabled={backups.length === 0 || running === true}>
          {t("game.restore")}
        </button>
        <button class="btn tall danger wide" onclick={openDelete} disabled={running === true}>
          {t("game.delete")}
        </button>
      </div>

      {#if game.cloudSteam}
        <div class="callout info">{t("game.cloudWarning")}</div>
      {/if}

      <section>
        <span class="caps">{t("game.backups", { n: backups.length })}</span>
        {#if backups.length === 0}
          <span class="muted">{t("game.noBackups")}</span>
        {/if}
        {#each backups.slice(0, 8) as b (b.path)}
          <div class="backup">
            <div class="backup-info">
              <span>{formatDate(b.createdAt)}</span>
              <span class="muted small">{formatSize(b.size)}{kindLabel(b)}</span>
            </div>
            <button class="btn small restore" onclick={() => restoreOne(b)} disabled={running === true}>
              {t("backup.restore")}
            </button>
          </div>
        {/each}
      </section>
    </div>
  {:else}
    <p class="none">{t("library.noSelection")}</p>
  {/if}
</aside>

{#if dialog === "delete" && game}
  <Dialog
    title={t("confirm.deleteTitle", { game: game.title })}
    icon="trash"
    tone="danger"
    confirmLabel={t("confirm.deleteAction")}
    onconfirm={remove}
    onclose={() => (dialog = null)}
  >
    {#each game.locations as loc (loc.path)}
      <div class="loc">
        <code class="mono">{loc.path}</code>
        <span class="muted small">
          {t("game.files", { n: loc.fileCount })} · {formatSize(loc.size)} · {t("game.modified", { when: relativeTime(loc.modified) })}
        </span>
      </div>
    {/each}
    <div class="check"><Icon name="check" color="var(--ok)" /><span>{t("confirm.deleteBackupFirst", { dir: app.config?.backupDir ?? "" })}</span></div>
    <div class="check"><Icon name="check" color="var(--ok)" /><span>{t("confirm.deleteTrash")}</span></div>
    {#if game.cloudSteam}
      <div class="callout info">{t("game.cloudWarning")}</div>
    {/if}
  </Dialog>
{/if}

{#if dialog === "restore" && game}
  <Dialog
    title={t("confirm.restoreTitle", { game: game.title })}
    icon="download"
    tone="accent"
    confirmLabel={t("confirm.restoreAction")}
    confirmDisabled={!restoreChoice}
    onconfirm={restore}
    onclose={() => (dialog = null)}
  >
    <span>{t("confirm.chooseBackup")}</span>
    <div class="choices">
      {#each backups as b (b.path)}
        <label class="choice">
          <input type="radio" name="backup" value={b.path} bind:group={restoreChoice} />
          <span>{formatDate(b.createdAt)}</span>
          <span class="muted small">{formatSize(b.size)}{kindLabel(b)}</span>
        </label>
      {/each}
    </div>
    <div class="check"><Icon name="check" color="var(--ok)" /><span>{t("confirm.restoreInfo")}</span></div>
    {#if game.cloudSteam}
      <div class="callout info">{t("game.cloudWarning")}</div>
    {/if}
  </Dialog>
{/if}

<style>
  aside {
    width: 360px;
    flex-shrink: 0;
    background: var(--panel);
    border-left: 1px solid var(--border);
    display: flex;
    flex-direction: column;
    overflow-y: auto;
  }

  .hero {
    position: relative;
    height: 170px;
    flex-shrink: 0;
    display: flex;
    align-items: flex-end;
  }

  .shade {
    position: absolute;
    inset: 0;
    background: linear-gradient(to top, rgb(14 16 20 / 0.95), rgb(14 16 20 / 0.1) 70%);
  }

  h1 {
    position: relative;
    margin: 0;
    padding: 18px 20px;
    font-family: var(--font-display);
    font-weight: 700;
    font-size: 26px;
    line-height: 1.1;
  }

  .content {
    padding: 16px 20px 24px;
    display: flex;
    flex-direction: column;
    gap: 18px;
  }

  .chips {
    display: flex;
    flex-wrap: wrap;
    gap: 6px;
  }

  section {
    display: flex;
    flex-direction: column;
    gap: 8px;
  }

  .loc {
    background: var(--surface);
    border: 1px solid #262b35;
    border-radius: var(--radius);
    padding: 10px 12px;
    display: flex;
    flex-direction: column;
    gap: 6px;
  }

  .loc-head {
    display: flex;
    gap: 8px;
    align-items: flex-start;
    justify-content: space-between;
  }

  .icon-btn {
    background: transparent;
    border: 0;
    color: var(--muted);
    padding: 2px;
    border-radius: 4px;
    display: flex;
  }

  .icon-btn:hover {
    color: var(--text);
  }

  .loc-meta {
    display: flex;
    justify-content: space-between;
    gap: 8px;
    color: var(--muted);
    font-size: 12px;
  }

  .actions {
    display: grid;
    grid-template-columns: repeat(2, minmax(0, 1fr));
    gap: 8px;
  }

  .wide {
    grid-column: span 2;
  }

  .backup {
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: 8px;
    padding: 8px 0;
    border-bottom: 1px solid var(--divider);
  }

  .backup-info {
    display: flex;
    flex-direction: column;
    gap: 2px;
  }

  .restore {
    background: transparent;
    color: var(--accent);
    border-color: var(--accent-border);
  }

  .small {
    font-size: 12px;
  }

  .none {
    color: var(--muted);
    padding: 24px;
    text-align: center;
  }

  .check {
    display: flex;
    gap: 10px;
    align-items: flex-start;
  }

  .choices {
    display: flex;
    flex-direction: column;
    gap: 4px;
  }

  .choice {
    display: grid;
    grid-template-columns: auto 1fr auto;
    gap: 10px;
    align-items: center;
    padding: 8px 10px;
    border-radius: 6px;
    background: var(--surface);
  }

  .choice input {
    accent-color: var(--accent);
  }
</style>
