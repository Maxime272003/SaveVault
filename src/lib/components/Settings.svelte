<script lang="ts">
  import { getVersion } from "@tauri-apps/api/app";
  import { open } from "@tauri-apps/plugin-dialog";
  import { openUrl } from "@tauri-apps/plugin-opener";
  import { relaunch } from "@tauri-apps/plugin-process";
  import { api, type CacheStats, type TitleHit } from "$lib/api";
  import { resetCoverCache } from "$lib/covers";
  import { formatSize, relativeTime } from "$lib/format";
  import { errorText, t } from "$lib/i18n.svelte";
  import { app } from "$lib/store.svelte";
  import Dialog from "./Dialog.svelte";

  const REPO = "https://github.com/Maxime272003/SaveVault";

  let version = $state("");
  let progress = $state<number | null>(null);
  let cache = $state<CacheStats | null>(null);
  let sgdbKey = $state(app.config?.steamgriddbKey ?? "");
  let updatingManifest = $state(false);

  let newPath = $state<string | null>(null);
  let query = $state("");
  let hits = $state<TitleHit[]>([]);

  const backupTotal = $derived(app.backups.reduce((s, b) => s + b.size, 0));
  const notes = $derived(
    (app.update?.body ?? "")
      .split("\n")
      .map((l) => l.replace(/^[-*]\s*/, "").trim())
      .filter((l) => l && !l.startsWith("#")),
  );

  $effect(() => {
    getVersion().then((v) => (version = v));
    api.coverCacheStats().then((c) => (cache = c));
  });

  $effect(() => {
    const q = query;
    if (newPath == null) return;
    const timer = setTimeout(async () => (hits = await api.searchTitles(q).catch(() => [])), 150);
    return () => clearTimeout(timer);
  });

  async function install() {
    if (!app.update) return;
    let total = 0;
    let done = 0;
    progress = 0;
    try {
      await app.update.downloadAndInstall((ev) => {
        if (ev.event === "Started") total = ev.data.contentLength ?? 0;
        if (ev.event === "Progress") {
          done += ev.data.chunkLength;
          progress = total ? Math.round((done / total) * 100) : null;
        }
      });
      await relaunch();
    } catch (e) {
      progress = null;
      app.toast(t("settings.updateError", { e: errorText(e) }), "error");
    }
  }

  async function chooseBackupDir() {
    const dir = await open({ directory: true, defaultPath: app.config?.backupDir ?? undefined });
    if (typeof dir === "string") {
      await app.saveConfig({ backupDir: dir }, true);
      await app.refreshBackups();
    }
  }

  async function addCustomPath() {
    const dir = await open({ directory: true });
    if (typeof dir === "string") {
      newPath = dir;
      query = dir.split(/[\\/]/).pop() ?? "";
    }
  }

  async function saveCustom(game: string) {
    if (!app.config || !newPath || !game.trim()) return;
    await app.saveConfig({ customEntries: [...app.config.customEntries, { game: game.trim(), path: newPath }] }, true);
    await app.rescan();
  }

  async function removeCustom(i: number) {
    if (!app.config) return;
    await app.saveConfig({ customEntries: app.config.customEntries.filter((_, j) => j !== i) });
    await app.rescan();
  }

  async function clearCache() {
    try {
      await api.clearCoverCache();
      resetCoverCache();
      cache = await api.coverCacheStats();
    } catch (e) {
      app.fail(e);
    }
  }

  async function updateManifest() {
    updatingManifest = true;
    try {
      app.manifest = await api.updateManifest(true);
      app.toast(t("settings.manifestUpdated"));
      await app.rescan();
    } catch (e) {
      app.fail(e);
    } finally {
      updatingManifest = false;
    }
  }
</script>

<div class="page">
  <h1>{t("settings.title")}</h1>

  <div class="grid">
    <section class="updates" class:has-update={app.update}>
      <div class="row">
        <h2>{t("settings.updates")}</h2>
        <span class="muted small">{t("settings.installed", { v: version })}</span>
      </div>

      {#if app.update}
        <div class="update">
          <div class="version">
            <span class="num">{app.update.version}</span>
            <span class="sub">
              {t("settings.available")}{#if app.update.date} · {t("settings.publishedOn", { date: app.update.date.slice(0, 10) })}{/if}
            </span>
          </div>
          {#if notes.length}
            <span class="caps notes-title">{t("settings.notes")}</span>
            <ul>
              {#each notes as line, i (i)}<li>{line}</li>{/each}
            </ul>
          {/if}
          <div class="row-left">
            <button class="btn tall info" onclick={install} disabled={progress !== null}>
              {progress !== null ? t("settings.installing", { p: progress != null ? `${progress} %` : "" }) : t("settings.install")}
            </button>
            <button class="btn ghost gh" onclick={() => openUrl(`${REPO}/releases`)}>{t("settings.viewGithub")}</button>
          </div>
        </div>
      {/if}

      <div class="row">
        <label class="check">
          <input
            type="checkbox"
            checked={app.config?.checkUpdatesOnStart}
            onchange={(e) => app.saveConfig({ checkUpdatesOnStart: e.currentTarget.checked })}
          />
          {t("settings.checkOnStart")}
        </label>
        <button class="btn" onclick={() => app.checkUpdate()} disabled={app.checkingUpdate}>
          {app.checkingUpdate ? t("settings.checking") : t("settings.check")}
        </button>
      </div>
    </section>

    <section>
      <h2>{t("settings.backups")}</h2>
      <label for="dir" class="muted small">{t("settings.backupDir")}</label>
      <div class="row-left">
        <input id="dir" class="input mono grow" readonly value={app.config?.backupDir ?? ""} />
        <button class="btn" onclick={chooseBackupDir}>{t("settings.change")}</button>
      </div>
      <div class="row-left wrap">
        <label class="check">
          <input
            type="checkbox"
            checked={app.config?.retentionEnabled}
            onchange={(e) => app.saveConfig({ retentionEnabled: e.currentTarget.checked })}
          />
          {t("settings.retentionBefore")}
        </label>
        <label for="keep" class="sr-only">{t("settings.retentionCount")}</label>
        <input
          id="keep"
          class="input count"
          type="number"
          min="1"
          max="999"
          value={app.config?.retentionCount ?? 5}
          onchange={(e) => app.saveConfig({ retentionCount: Math.max(1, Number(e.currentTarget.value) || 5) })}
        />
        <span>{t("settings.retentionAfter")}</span>
      </div>
      <div class="row foot">
        <span class="muted">{t("settings.backupStats", { n: app.backups.length })}</span>
        <strong>{t("settings.used", { size: formatSize(backupTotal) })}</strong>
      </div>
    </section>

    <section>
      <h2>{t("settings.general")}</h2>
      <div class="row">
        <span>{t("settings.language")}</span>
        <div class="segmented" role="group" aria-label={t("settings.language")}>
          <button aria-pressed={app.config?.language === "fr"} onclick={() => app.saveConfig({ language: "fr" })}>Français</button>
          <button aria-pressed={app.config?.language === "en"} onclick={() => app.saveConfig({ language: "en" })}>English</button>
        </div>
      </div>
      <div class="row">
        <span>{t("settings.customPaths")}</span>
        <button class="btn" onclick={addCustomPath}>{t("settings.addPath")}</button>
      </div>
      {#if app.config?.customEntries.length}
        <ul class="custom">
          {#each app.config.customEntries as entry, i (entry.path + entry.game)}
            <li>
              <div class="custom-info">
                <span>{entry.game}</span>
                <code class="mono">{entry.path}</code>
              </div>
              <button class="btn small ghost" onclick={() => removeCustom(i)}>{t("settings.remove")}</button>
            </li>
          {/each}
        </ul>
      {:else}
        <span class="muted small">{t("settings.noCustom")}</span>
      {/if}
      {#if app.config?.ignoredPaths.length}
        <div class="row">
          <span class="muted small">{t("settings.ignoredCount", { n: app.config.ignoredPaths.length })}</span>
          <button
            class="btn small ghost"
            onclick={async () => {
              await app.saveConfig({ ignoredPaths: [] });
              await app.rescan();
            }}>{t("settings.unignore")}</button
          >
        </div>
      {/if}
    </section>

    <section>
      <h2>{t("settings.images")}</h2>
      <label for="sgdb" class="muted small">{t("settings.sgdbKey")}</label>
      <input
        id="sgdb"
        class="input"
        type="password"
        placeholder={t("settings.sgdbPlaceholder")}
        bind:value={sgdbKey}
        onchange={() => app.saveConfig({ steamgriddbKey: sgdbKey.trim() }, true)}
      />
      <div class="row">
        <span class="muted small">{cache ? t("settings.cache", { n: cache.count, size: formatSize(cache.bytes) }) : ""}</span>
        <button class="btn small" onclick={clearCache}>{t("settings.clearCache")}</button>
      </div>
    </section>

    <section>
      <h2>{t("settings.database")}</h2>
      <span class="muted small">
        {#if app.manifest?.updatedAt}
          {t("settings.manifest", { n: app.manifest.gameCount.toLocaleString(), when: relativeTime(app.manifest.updatedAt) })}
        {:else}
          {t("settings.manifestMissing")}
        {/if}
      </span>
      <div>
        <button class="btn" onclick={updateManifest} disabled={updatingManifest || app.scanning}>{t("settings.updateManifest")}</button>
      </div>
    </section>
  </div>
</div>

{#if newPath != null}
  {@const path = newPath}
  <Dialog
    title={t("custom.nameTitle")}
    icon="folder"
    tone="accent"
    confirmLabel={t("unknown.attach")}
    confirmDisabled={!query.trim()}
    onconfirm={() => saveCustom(query)}
    onclose={() => (newPath = null)}
  >
    <code class="mono">{path}</code>
    <label class="sr-only" for="custom-q">{t("unknown.searchGame")}</label>
    <input id="custom-q" class="input" placeholder={t("unknown.searchGame")} bind:value={query} />
    <div class="hits">
      {#each hits as h (h.name)}
        <button class="hit" class:on={query === h.name} onclick={() => (query = h.name)}>{h.name}</button>
      {/each}
    </div>
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
    margin: 0 0 24px;
    font-family: var(--font-display);
    font-size: 28px;
  }

  h2 {
    margin: 0;
    font-size: 16px;
    font-weight: 600;
  }

  .grid {
    display: grid;
    grid-template-columns: repeat(auto-fit, minmax(380px, 1fr));
    gap: 20px;
    align-items: start;
  }

  section {
    background: var(--panel);
    border: 1px solid var(--border);
    border-radius: 12px;
    padding: 20px;
    display: flex;
    flex-direction: column;
    gap: 14px;
  }

  section.has-update {
    border-color: #2a3a52;
  }

  .row {
    display: flex;
    justify-content: space-between;
    align-items: center;
    gap: 12px;
  }

  .row-left {
    display: flex;
    align-items: center;
    gap: 8px;
  }

  .wrap {
    flex-wrap: wrap;
  }

  .grow {
    flex: 1;
  }

  .foot {
    padding-top: 10px;
    border-top: 1px solid var(--divider);
  }

  .small {
    font-size: 12px;
  }

  .check {
    display: flex;
    gap: 8px;
    align-items: center;
  }

  .check input {
    accent-color: var(--accent);
  }

  .count {
    width: 70px;
  }

  .update {
    background: var(--info-bg);
    border: 1px solid var(--info-border);
    border-radius: 10px;
    padding: 16px;
    display: flex;
    flex-direction: column;
    gap: 10px;
  }

  .version {
    display: flex;
    align-items: baseline;
    gap: 10px;
  }

  .num {
    font-family: var(--font-display);
    font-size: 22px;
    font-weight: 700;
    color: #e8f2fc;
  }

  .sub,
  .notes-title {
    font-size: 12px;
    color: #9ccbf7;
  }

  .update ul {
    margin: 0;
    padding-left: 18px;
    display: flex;
    flex-direction: column;
    gap: 6px;
    color: #d5e4f3;
    line-height: 1.4;
  }

  .gh {
    color: #9ccbf7;
    padding: 0 8px;
  }

  .segmented {
    display: flex;
    background: var(--surface);
    border: 1px solid var(--border-strong);
    border-radius: var(--radius);
    padding: 3px;
  }

  .segmented button {
    height: 34px;
    padding: 0 14px;
    background: transparent;
    color: var(--muted);
    border: 0;
    border-radius: 6px;
    font-weight: 600;
  }

  .segmented button[aria-pressed="true"] {
    background: #2a303b;
    color: var(--text);
  }

  .custom {
    list-style: none;
    margin: 0;
    padding: 0;
    display: flex;
    flex-direction: column;
    gap: 6px;
  }

  .custom li {
    display: flex;
    justify-content: space-between;
    align-items: center;
    gap: 8px;
    background: var(--surface);
    border-radius: 6px;
    padding: 8px 10px;
  }

  .custom-info {
    display: flex;
    flex-direction: column;
    gap: 2px;
    min-width: 0;
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
</style>
