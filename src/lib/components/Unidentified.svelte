<script lang="ts">
  import { api, type TitleHit, type UnknownDir } from "$lib/api";
  import { formatSize, relativeTime } from "$lib/format";
  import { t } from "$lib/i18n.svelte";
  import { app } from "$lib/store.svelte";
  import Dialog from "./Dialog.svelte";
  import Icon from "./Icon.svelte";

  const list = $derived(app.scan?.unidentified ?? []);

  let attaching = $state<UnknownDir | null>(null);
  let deleting = $state<UnknownDir | null>(null);
  let query = $state("");
  let hits = $state<TitleHit[]>([]);
  let picked = $state("");

  $effect(() => {
    const q = query;
    if (!attaching) return;
    const timer = setTimeout(async () => {
      hits = await api.searchTitles(q).catch(() => []);
    }, 150);
    return () => clearTimeout(timer);
  });

  function startAttach(u: UnknownDir) {
    attaching = u;
    query = u.suggestion ?? u.name.split(" / ").pop() ?? "";
    picked = u.suggestion ?? "";
  }

  async function attach(u: UnknownDir, game: string) {
    if (!app.config || !game.trim()) return;
    await app.saveConfig({ customEntries: [...app.config.customEntries, { game: game.trim(), path: u.path }] });
    app.toast(t("unknown.attached", { game: game.trim() }));
    await app.rescan();
  }

  async function ignore(u: UnknownDir) {
    if (!app.config) return;
    await app.saveConfig({ ignoredPaths: [...app.config.ignoredPaths, u.path] });
    if (app.scan) app.scan.unidentified = app.scan.unidentified.filter((x) => x.path !== u.path);
    app.toast(t("unknown.ignored"));
  }

  async function backup(u: UnknownDir) {
    if (!app.config?.backupDir) {
      app.view = "settings";
      app.toast(t("err.no_backup_dir"), "error");
      return;
    }
    try {
      const info = await api.backupUnknown(u.path);
      app.toast(t("backup.created", { name: info.fileName }));
      await app.refreshBackups();
    } catch (e) {
      app.fail(e);
    }
  }

  async function remove(u: UnknownDir) {
    try {
      await api.deleteUnknown(u.path);
      if (app.scan) app.scan.unidentified = app.scan.unidentified.filter((x) => x.path !== u.path);
      await app.refreshBackups();
    } catch (e) {
      app.fail(e);
    }
  }
</script>

<div class="page">
  <h1>{t("unknown.title")}</h1>
  <p class="muted intro">{t("unknown.intro")}</p>

  {#if list.length === 0}
    <p class="muted">{t("unknown.empty")}</p>
  {/if}

  <ul>
    {#each list as u (u.path)}
      <li>
        <div class="info">
          <span class="name">{u.name}</span>
          <code class="mono">{u.path}</code>
          <span class="muted small">
            {formatSize(u.size)} · {t("game.files", { n: u.fileCount })} · {t("game.modified", { when: relativeTime(u.modified) })}
            {#if u.hint}· {u.hint}{/if}
          </span>
        </div>
        <div class="actions">
          {#if u.suggestion}
            <button class="btn small primary" onclick={() => attach(u, u.suggestion!)}>
              <Icon name="link" size={14} />
              {u.suggestion}
            </button>
          {/if}
          <button class="btn small" onclick={() => startAttach(u)}>{t("unknown.attach")}</button>
          <button class="btn small" onclick={() => api.openPath(u.path).catch((e) => app.fail(e))} aria-label={t("game.open")} title={t("game.open")}>
            <Icon name="folder" size={14} />
          </button>
          <button class="btn small" onclick={() => backup(u)} aria-label={t("game.backup")} title={t("game.backup")}>
            <Icon name="download" size={14} />
          </button>
          <button class="btn small ghost" onclick={() => ignore(u)}>{t("unknown.ignore")}</button>
          <button class="btn small danger" onclick={() => (deleting = u)} aria-label={t("backup.delete")} title={t("backup.delete")}>
            <Icon name="trash" size={14} />
          </button>
        </div>
      </li>
    {/each}
  </ul>
</div>

{#if attaching}
  {@const u = attaching}
  <Dialog
    title={t("custom.nameTitle")}
    icon="folder"
    tone="accent"
    confirmLabel={t("unknown.attach")}
    confirmDisabled={!picked.trim()}
    onconfirm={() => attach(u, picked)}
    onclose={() => (attaching = null)}
  >
    <code class="mono">{u.path}</code>
    <label class="sr-only" for="game-q">{t("unknown.searchGame")}</label>
    <input id="game-q" class="input" placeholder={t("unknown.searchGame")} bind:value={query} oninput={() => (picked = query)} />
    <div class="hits">
      {#each hits as h (h.name)}
        <button class="hit" class:on={picked === h.name} onclick={() => (picked = h.name)}>{h.name}</button>
      {/each}
      {#if query.trim() && !hits.some((h) => h.name === query.trim())}
        <button class="hit" class:on={picked === query.trim()} onclick={() => (picked = query.trim())}>
          {t("unknown.useName", { name: query.trim() })}
        </button>
      {/if}
    </div>
  </Dialog>
{/if}

{#if deleting}
  {@const u = deleting}
  <Dialog
    title={t("confirm.deleteUnknownTitle", { name: u.name })}
    icon="trash"
    tone="danger"
    confirmLabel={t("confirm.deleteAction")}
    onconfirm={() => remove(u)}
    onclose={() => (deleting = null)}
  >
    <code class="mono">{u.path}</code>
    <span class="muted small">{t("game.files", { n: u.fileCount })} · {formatSize(u.size)}</span>
    <div class="check"><Icon name="check" color="var(--ok)" /><span>{t("confirm.deleteBackupFirst", { dir: app.config?.backupDir ?? "" })}</span></div>
    <div class="check"><Icon name="check" color="var(--ok)" /><span>{t("confirm.deleteTrash")}</span></div>
  </Dialog>
{/if}

<style>
  .page {
    flex: 1;
    min-width: 0;
    overflow-y: auto;
    padding: 32px 40px;
  }

  h1 {
    margin: 0 0 8px;
    font-family: var(--font-display);
    font-size: 28px;
  }

  .intro {
    margin: 0 0 24px;
    max-width: 640px;
    line-height: 1.5;
  }

  ul {
    list-style: none;
    margin: 0;
    padding: 0;
    display: flex;
    flex-direction: column;
    gap: 8px;
  }

  li {
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: 16px;
    padding: 14px 16px;
    background: var(--panel);
    border: 1px solid var(--border);
    border-radius: 10px;
    flex-wrap: wrap;
  }

  .info {
    display: flex;
    flex-direction: column;
    gap: 4px;
    min-width: 0;
    flex: 1 1 360px;
  }

  .name {
    font-weight: 600;
  }

  .small {
    font-size: 12px;
  }

  .actions {
    display: flex;
    gap: 6px;
    flex-wrap: wrap;
  }

  .hits {
    display: flex;
    flex-direction: column;
    gap: 2px;
    max-height: 260px;
    overflow-y: auto;
  }

  .hit {
    text-align: left;
    background: transparent;
    border: 0;
    padding: 8px 10px;
    border-radius: 6px;
  }

  .hit:hover {
    background: var(--surface);
  }

  .hit.on {
    background: #3a2d14;
    color: var(--accent);
  }

  .check {
    display: flex;
    gap: 10px;
    align-items: flex-start;
  }
</style>
