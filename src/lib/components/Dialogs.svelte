<script lang="ts">
  import { api, type BackupInfo } from "$lib/api";
  import { formatDate, formatSize, relativeTime } from "$lib/format";
  import { t } from "$lib/i18n.svelte";
  import { app } from "$lib/store.svelte";
  import Dialog from "./Dialog.svelte";
  import Icon from "./Icon.svelte";

  const d = $derived(app.dialog);
  const game = $derived(d && (d.kind === "delete" || d.kind === "restore") ? app.game(d.gameId) : undefined);
  const backups = $derived(game ? app.backupsFor(game.title) : []);

  let restoreChoice = $state<string | null>(null);
  let onlyChanged = $state(true);
  let steamOpen = $state(false);

  // À chaque ouverture : présélection du backup et vérification de Steam pour les jeux Steam Cloud.
  $effect(() => {
    if (d?.kind === "restore") restoreChoice = d.backupPath ?? backups[0]?.path ?? null;
    if (d?.kind === "batch") onlyChanged = true;
    steamOpen = false;
    if (game?.cloudSteam) api.isSteamRunning().then((r) => (steamOpen = r)).catch(() => {});
  });

  const close = () => (app.dialog = null);
  const kindLabel = (b: BackupInfo) => (b.kind === "manual" ? "" : ` · ${t(`backup.kind.${b.kind}`)}`);
  const sizeOf = (ids: string[]) => ids.reduce((s, id) => s + (app.game(id)?.size ?? 0), 0);
</script>

{#if d?.kind === "delete" && game}
  <Dialog
    title={t("confirm.deleteTitle", { game: game.title })}
    icon="trash"
    tone="danger"
    confirmLabel={t("confirm.deleteAction")}
    onconfirm={() => app.deleteGame(game.id)}
    onclose={close}
  >
    {#each game.locations as loc (loc.path)}
      <div class="loc">
        <code class="mono">{loc.path}</code>
        <span class="muted small">
          {#if loc.kind === "registry"}
            {t("game.registry")} · {t("game.entries", { n: loc.fileCount })}
          {:else}
            {t("game.files", { n: loc.fileCount })} · {formatSize(loc.size)} · {t("game.modified", { when: relativeTime(loc.modified) })}
          {/if}
        </span>
      </div>
    {/each}
    <div class="check"><Icon name="check" color="var(--ok)" /><span>{t("confirm.deleteBackupFirst", { dir: app.config?.backupDir ?? "" })}</span></div>
    <div class="check"><Icon name="check" color="var(--ok)" /><span>{t("confirm.deleteTrash")}</span></div>
    {#if game.cloudSteam}<div class="callout info">{t("game.cloudWarning")}</div>{/if}
    {#if steamOpen}<div class="callout danger">{t("steam.running")}</div>{/if}
  </Dialog>
{:else if d?.kind === "restore" && game}
  <Dialog
    title={t("confirm.restoreTitle", { game: game.title })}
    icon="download"
    tone="accent"
    confirmLabel={t("confirm.restoreAction")}
    confirmDisabled={!restoreChoice}
    onconfirm={() => (restoreChoice ? app.restore(restoreChoice) : undefined)}
    onclose={close}
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
    {#if game.cloudSteam}<div class="callout info">{t("game.cloudWarning")}</div>{/if}
    {#if steamOpen}<div class="callout danger">{t("steam.running")}</div>{/if}
  </Dialog>
{:else if d?.kind === "batch"}
  {@const ids = d.gameIds}
  <Dialog
    title={t("batch.title", { n: ids.length })}
    icon="download"
    tone="accent"
    confirmLabel={t("batch.action")}
    onconfirm={() => {
      // Les backups tournent en arrière-plan ; la barre de progression les suit.
      void app.backupMany(ids, onlyChanged);
    }}
    onclose={close}
  >
    <span class="muted">{formatSize(sizeOf(ids))} · {app.config?.backupDir ?? ""}</span>
    <label class="check"><input type="checkbox" bind:checked={onlyChanged} /> {t("batch.onlyChanged")}</label>
  </Dialog>
{:else if d?.kind === "clean"}
  {@const ids = d.gameIds}
  <Dialog
    title={t("disk.cleanTitle", { n: ids.length })}
    icon="trash"
    tone="danger"
    confirmLabel={t("confirm.deleteAction")}
    onconfirm={() => app.deleteGames(ids)}
    onclose={close}
  >
    <ul class="list">
      {#each ids as id (id)}
        {@const g = app.game(id)}
        {#if g}<li><span>{g.title}</span><span class="muted">{formatSize(g.size)}</span></li>{/if}
      {/each}
    </ul>
    <div class="check"><Icon name="check" color="var(--ok)" /><span>{t("confirm.deleteBackupFirst", { dir: app.config?.backupDir ?? "" })}</span></div>
    <div class="check"><Icon name="check" color="var(--ok)" /><span>{t("confirm.deleteTrash")}</span></div>
  </Dialog>
{/if}

<style>
  .loc {
    background: var(--surface);
    border: 1px solid #262b35;
    border-radius: var(--radius);
    padding: 10px 12px;
    display: flex;
    flex-direction: column;
    gap: 6px;
  }

  .small {
    font-size: 12px;
  }

  .check {
    display: flex;
    gap: 10px;
    align-items: flex-start;
  }

  .check input {
    accent-color: var(--accent);
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

  .list {
    list-style: none;
    margin: 0;
    padding: 0;
    display: flex;
    flex-direction: column;
    gap: 4px;
  }

  .list li {
    display: flex;
    justify-content: space-between;
    gap: 12px;
    padding: 6px 10px;
    background: var(--surface);
    border-radius: 6px;
  }
</style>
