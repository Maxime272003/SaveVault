<script lang="ts">
  import { api, type BackupInfo, type FileList, type Location } from "$lib/api";
  import { formatDate, formatSize, relativeTime } from "$lib/format";
  import { t } from "$lib/i18n.svelte";
  import { app } from "$lib/store.svelte";
  import Cover from "./Cover.svelte";
  import Icon from "./Icon.svelte";

  const game = $derived(app.selected);
  const backups = $derived(game ? app.backupsFor(game.title) : []);
  /** Dossiers non identifiés dont le nom correspond à ce jeu : à fusionner avec lui. */
  const linkable = $derived(game ? (app.scan?.unidentified ?? []).filter((u) => u.suggestion === game.title) : []);

  let running = $state<boolean | null>(null);
  let busy = $state(false);
  let openFiles = $state<Record<string, FileList | "loading">>({});

  $effect(() => {
    const id = game?.id;
    running = null;
    openFiles = {};
    if (!id) return;
    let cancelled = false;
    api.isGameRunning(id).then((r) => {
      if (!cancelled) running = r;
    });
    return () => {
      cancelled = true;
    };
  });

  async function backup() {
    if (!game) return;
    busy = true;
    await app.backupGame(game.id);
    busy = false;
  }

  async function toggleFiles(loc: Location) {
    if (openFiles[loc.path]) {
      const { [loc.path]: _, ...rest } = openFiles;
      openFiles = rest;
      return;
    }
    openFiles = { ...openFiles, [loc.path]: "loading" };
    try {
      const list = await api.listLocationFiles(loc.path, loc.kind === "files" ? loc.files : []);
      openFiles = { ...openFiles, [loc.path]: list };
    } catch (e) {
      app.fail(e);
    }
  }

  function openRestore(b?: BackupInfo) {
    if (!game || !app.requireBackupDir()) return;
    app.dialog = { kind: "restore", gameId: game.id, backupPath: b?.path };
  }

  function openDelete() {
    if (!game || !app.requireBackupDir()) return;
    app.dialog = { kind: "delete", gameId: game.id };
  }

  const firstFolder = $derived(game?.locations.find((l) => l.kind !== "registry")?.path ?? null);
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
          {@const files = openFiles[loc.path]}
          <div class="loc">
            <div class="loc-head">
              <code class="mono">{loc.path}</code>
              {#if loc.kind !== "registry"}
                <button
                  class="icon-btn"
                  aria-label={t("game.openLocation")}
                  title={t("game.openLocation")}
                  onclick={() => api.openPath(loc.path).catch((e) => app.fail(e))}
                >
                  <Icon name="folder" />
                </button>
              {/if}
            </div>
            <div class="loc-meta">
              {#if loc.kind === "registry"}
                <span>{t("game.registry")} · {t("game.entries", { n: loc.fileCount })}</span>
              {:else}
                <span>
                  {formatSize(loc.size)} · {t("game.files", { n: loc.fileCount })}
                  {#if loc.custom}· {t("game.custom")}{/if}
                </span>
                <span>{t("game.modified", { when: relativeTime(loc.modified) })}</span>
              {/if}
            </div>
            {#if loc.kind !== "registry"}
              <button class="link" onclick={() => toggleFiles(loc)} aria-expanded={!!files}>
                {files ? t("game.hideFiles") : t("game.showFiles")}
              </button>
              {#if files === "loading"}
                <span class="muted small">…</span>
              {:else if files}
                <ul class="files">
                  {#each files.files as f (f.path)}
                    <li>
                      <span class="fname" title={f.path}>{f.path}</span>
                      <span class="muted">{formatSize(f.size)}</span>
                      <span class="muted">{relativeTime(f.modified)}</span>
                    </li>
                  {/each}
                </ul>
                {#if files.truncated}<span class="muted small">{t("game.filesTruncated")}</span>{/if}
              {/if}
            {/if}
          </div>
        {/each}
      </section>

      {#if linkable.length}
        <section class="linkable">
          <span class="caps">{t("game.linkable", { n: linkable.length })}</span>
          <span class="muted small">{t("game.linkableHint")}</span>
          {#each linkable as u (u.path)}
            <div class="link-row">
              <code class="mono">{u.path}</code>
              <span class="muted small">{formatSize(u.size)}</span>
            </div>
          {/each}
          <button class="btn small primary" onclick={() => app.attach(linkable.map((u) => ({ game: game.title, path: u.path })))}>
            <Icon name="link" size={14} />
            {t("game.attachAll")}
          </button>
        </section>
      {/if}

      <div class="actions">
        <button class="btn tall primary wide" onclick={backup} disabled={busy}>
          <Icon name="download" />
          {t("game.backup")}
        </button>
        <button class="btn tall" onclick={() => firstFolder && api.openPath(firstFolder).catch((e) => app.fail(e))} disabled={!firstFolder}>
          {t("game.open")}
        </button>
        <button class="btn tall" onclick={() => openRestore()} disabled={backups.length === 0 || running === true}>
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
            <button class="btn small restore" onclick={() => openRestore(b)} disabled={running === true}>
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

  .link {
    align-self: flex-start;
    background: transparent;
    border: 0;
    padding: 0;
    color: var(--accent);
    font-size: 12px;
    font-weight: 600;
  }

  .files {
    list-style: none;
    margin: 0;
    padding: 6px 0 0;
    border-top: 1px solid var(--divider);
    max-height: 220px;
    overflow-y: auto;
    display: flex;
    flex-direction: column;
    gap: 3px;
    font-size: 12px;
  }

  .files li {
    display: grid;
    grid-template-columns: minmax(0, 1fr) auto auto;
    gap: 8px;
  }

  .fname {
    font-family: var(--font-mono);
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }

  .linkable {
    background: #1c1a14;
    border: 1px solid var(--accent-border);
    border-radius: var(--radius);
    padding: 12px;
  }

  .link-row {
    display: flex;
    justify-content: space-between;
    gap: 8px;
  }

  .linkable .btn {
    align-self: flex-start;
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
</style>
