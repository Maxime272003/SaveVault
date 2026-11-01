<script lang="ts">
  import { api, type Game } from "$lib/api";
  import { formatSize } from "$lib/format";
  import { t } from "$lib/i18n.svelte";
  import { app } from "$lib/store.svelte";
  import ContextMenu, { type MenuItem } from "./ContextMenu.svelte";
  import Cover from "./Cover.svelte";
  import GameDetail from "./GameDetail.svelte";
  import Icon from "./Icon.svelte";

  let grid = $state<HTMLDivElement>();
  let searchInput: HTMLInputElement;
  let menu = $state<{ x: number; y: number; game: Game } | null>(null);
  /** Point de départ d'une sélection Maj + clic. */
  let anchor: string | null = null;

  function select(game: Game, e: MouseEvent | KeyboardEvent) {
    if (e.ctrlKey || e.metaKey) {
      if (app.multi.has(game.id)) app.multi.delete(game.id);
      else app.multi.add(game.id);
      if (app.multi.size === 1 && app.selectedId && app.selectedId !== game.id) app.multi.add(app.selectedId);
    } else if (e.shiftKey && anchor) {
      const ids = app.filtered.map((g) => g.id);
      const [a, b] = [ids.indexOf(anchor), ids.indexOf(game.id)].sort((x, y) => x - y);
      app.multi.clear();
      for (const id of ids.slice(a, b + 1)) app.multi.add(id);
    } else {
      app.multi.clear();
      anchor = game.id;
    }
    app.selectedId = game.id;
  }

  function openMenu(e: MouseEvent, game: Game) {
    e.preventDefault();
    if (!app.multi.has(game.id)) {
      app.multi.clear();
      app.selectedId = game.id;
    }
    menu = { x: e.clientX, y: e.clientY, game };
  }

  function menuItems(game: Game): MenuItem[] {
    if (app.multi.size > 1) {
      const ids = [...app.multi];
      return [
        { label: t("library.backupSelection", { n: ids.length }), action: () => openBatch(ids) },
        { label: t("library.clearSelection"), action: () => app.multi.clear() },
      ];
    }
    const folder = game.locations.find((l) => l.kind !== "registry")?.path;
    return [
      { label: t("game.backup"), shortcut: "Ctrl+B", action: () => app.backupGame(game.id) },
      { label: t("game.open"), disabled: !folder, action: () => folder && api.openPath(folder).catch((e) => app.fail(e)) },
      {
        label: t("game.restore"),
        disabled: app.backupsFor(game.title).length === 0,
        action: () => app.requireBackupDir() && (app.dialog = { kind: "restore", gameId: game.id }),
      },
      {
        label: t("game.delete"),
        shortcut: t("keys.deleteKey"),
        danger: true,
        action: () => app.requireBackupDir() && (app.dialog = { kind: "delete", gameId: game.id }),
      },
    ];
  }

  function openBatch(ids: string[]) {
    if (app.requireBackupDir()) app.dialog = { kind: "batch", gameIds: ids };
  }

  /** Nombre de colonnes réellement affichées par la grille (dépend de la largeur de la fenêtre). */
  function columns(): number {
    if (!grid) return 1;
    return getComputedStyle(grid).gridTemplateColumns.split(" ").filter(Boolean).length || 1;
  }

  function move(delta: number) {
    const list = app.filtered;
    if (!list.length) return;
    const i = list.findIndex((g) => g.id === app.selectedId);
    const next = list[Math.max(0, Math.min(list.length - 1, (i < 0 ? 0 : i) + delta))];
    app.multi.clear();
    app.selectedId = next.id;
    anchor = next.id;
    grid?.querySelector(`[data-id="${CSS.escape(next.id)}"]`)?.scrollIntoView({ block: "nearest" });
  }

  function onkeydown(e: KeyboardEvent) {
    if (app.dialog || menu) return;
    const typing = (e.target as HTMLElement)?.closest("input, select, textarea");
    const ctrl = e.ctrlKey || e.metaKey;
    if (ctrl && e.key.toLowerCase() === "f") {
      e.preventDefault();
      searchInput.focus();
      searchInput.select();
      return;
    }
    if (e.key === "F5" || (ctrl && e.key.toLowerCase() === "r")) {
      e.preventDefault();
      void app.rescan();
      return;
    }
    if (typing) {
      if (e.key === "Escape") (e.target as HTMLElement).blur();
      return;
    }
    const game = app.selected;
    if (ctrl && e.key.toLowerCase() === "a") {
      e.preventDefault();
      for (const g of app.filtered) app.multi.add(g.id);
    } else if (ctrl && e.key.toLowerCase() === "b") {
      e.preventDefault();
      if (app.multi.size > 1) openBatch([...app.multi]);
      else if (game) void app.backupGame(game.id);
    } else if (e.key === "Delete" && game) {
      e.preventDefault();
      if (app.requireBackupDir()) app.dialog = { kind: "delete", gameId: game.id };
    } else if (e.key === "Escape") {
      app.multi.clear();
    } else if (e.key === "ArrowRight") {
      e.preventDefault();
      move(1);
    } else if (e.key === "ArrowLeft") {
      e.preventDefault();
      move(-1);
    } else if (e.key === "ArrowDown") {
      e.preventDefault();
      move(columns());
    } else if (e.key === "ArrowUp") {
      e.preventDefault();
      move(-columns());
    } else if (e.key === "ContextMenu" && game) {
      e.preventDefault();
      const el = grid?.querySelector(`[data-id="${CSS.escape(game.id)}"]`)?.getBoundingClientRect();
      if (el) menu = { x: el.left + el.width / 2, y: el.top + el.height / 2, game };
    }
  }
</script>

<svelte:window {onkeydown} />

<div class="library">
  <main>
    <div class="toolbar">
      <label for="q" class="sr-only">{t("library.search")}</label>
      <div class="search">
        <Icon name="search" color="var(--muted)" />
        <input id="q" bind:this={searchInput} placeholder={t("library.search")} bind:value={app.search} />
        <kbd>Ctrl+F</kbd>
      </div>
      <select class="input" aria-label={t("library.sort")} bind:value={app.sort}>
        <option value="modified">{t("library.sortModified")}</option>
        <option value="name">{t("library.sortName")}</option>
        <option value="size">{t("library.sortSize")}</option>
      </select>
      {#if app.multi.size > 1}
        <button class="btn primary" onclick={() => openBatch([...app.multi])}>
          <Icon name="download" />
          {t("library.backupSelection", { n: app.multi.size })}
        </button>
        <button class="btn ghost" onclick={() => app.multi.clear()}>{t("library.clearSelection")}</button>
      {:else}
        <button class="btn" onclick={() => openBatch(app.filtered.map((g) => g.id))} disabled={!app.filtered.length}>
          <Icon name="download" />
          {t("library.backupAll")}
        </button>
      {/if}
      <button class="btn" onclick={() => app.rescan()} disabled={app.scanning} title="F5">
        <span class:spin={app.scanning}><Icon name="refresh" /></span>
        {app.scanning ? t("library.scanning") : t("library.rescan")}
      </button>
    </div>

    <div class="scroll">
      {#if !app.scan}
        <p class="empty">{t("library.firstScan")}</p>
      {:else if app.filtered.length === 0}
        <p class="empty">{t("library.empty")}</p>
      {:else}
        <div class="grid" bind:this={grid}>
          {#each app.filtered as game (game.id)}
            <button
              class="card"
              class:selected={game.id === app.selectedId}
              class:multi={app.multi.has(game.id)}
              data-id={game.id}
              onclick={(e) => select(game, e)}
              oncontextmenu={(e) => openMenu(e, game)}
              aria-label={game.title}
              aria-pressed={game.id === app.selectedId || app.multi.has(game.id)}
            >
              <div class="art">
                <Cover appId={game.steamAppId} title={game.title} />
                <div class="badges">
                  <span class="source">{t(`source.${game.source}`)}</span>
                  {#if app.multi.has(game.id)}
                    <span class="tick"><Icon name="check" size={14} color="var(--accent-ink)" /></span>
                  {:else if game.cloudSteam}
                    <span class="cloud" title="Steam Cloud"><Icon name="cloud" size={14} color="var(--info)" label="Steam Cloud" /></span>
                  {/if}
                </div>
              </div>
              <div class="meta">
                <span class="title">{game.title}</span>
                <span class="line">
                  <span>{formatSize(game.size)}</span>
                  {#if !game.installed}<span class="warn">{t("filter.notInstalled")}</span>{/if}
                </span>
              </div>
            </button>
          {/each}
        </div>
      {/if}
    </div>
  </main>

  <GameDetail />
</div>

{#if menu}
  <ContextMenu x={menu.x} y={menu.y} items={menuItems(menu.game)} onclose={() => (menu = null)} />
{/if}

<style>
  .library {
    flex: 1;
    min-width: 0;
    display: flex;
  }

  main {
    flex: 1;
    min-width: 0;
    display: flex;
    flex-direction: column;
  }

  .toolbar {
    display: flex;
    align-items: center;
    gap: 10px;
    padding: 18px 24px;
    border-bottom: 1px solid #1e232b;
    flex-wrap: wrap;
  }

  .search {
    flex: 1 1 200px;
    display: flex;
    align-items: center;
    gap: 8px;
    background: #171b22;
    border: 1px solid var(--border-strong);
    border-radius: var(--radius);
    padding: 0 12px;
    height: 40px;
  }

  .search input {
    flex: 1;
    background: transparent;
    border: 0;
    outline: none;
    min-width: 0;
  }

  kbd {
    font-family: var(--font-body);
    font-size: 11px;
    color: var(--muted);
    border: 1px solid var(--border-strong);
    border-radius: 4px;
    padding: 1px 5px;
  }

  .spin {
    display: inline-flex;
    animation: spin 1s linear infinite;
  }

  @keyframes spin {
    to {
      transform: rotate(360deg);
    }
  }

  .scroll {
    flex: 1;
    overflow-y: auto;
    padding: 20px 24px;
  }

  .empty {
    color: var(--muted);
    margin: 40px auto;
    text-align: center;
    max-width: 420px;
    line-height: 1.5;
  }

  .grid {
    display: grid;
    grid-template-columns: repeat(auto-fill, minmax(140px, 1fr));
    gap: 16px 14px;
  }

  .card {
    padding: 0;
    border: 0;
    background: transparent;
    text-align: left;
    color: var(--text);
    display: flex;
    flex-direction: column;
    gap: 8px;
    border-radius: var(--radius);
  }

  .art {
    position: relative;
    aspect-ratio: 2 / 3;
    border-radius: var(--radius);
    overflow: hidden;
    outline: 2px solid transparent;
    transition:
      outline-color 0.12s,
      transform 0.12s;
  }

  .card:hover .art {
    transform: translateY(-2px);
  }

  .card.selected .art,
  .card.multi .art {
    outline-color: var(--accent);
  }

  .card.multi:not(.selected) .art {
    outline-style: dashed;
  }

  .badges {
    position: absolute;
    top: 8px;
    left: 8px;
    right: 8px;
    display: flex;
    justify-content: space-between;
    align-items: flex-start;
    pointer-events: none;
  }

  .source {
    font-size: 10px;
    font-weight: 600;
    letter-spacing: 0.06em;
    text-transform: uppercase;
    background: rgb(0 0 0 / 0.6);
    padding: 3px 6px;
    border-radius: 4px;
  }

  .cloud {
    background: rgb(0 0 0 / 0.6);
    border-radius: 4px;
    padding: 3px 4px;
    display: flex;
  }

  .tick {
    background: var(--accent);
    border-radius: 50%;
    width: 22px;
    height: 22px;
    display: flex;
    align-items: center;
    justify-content: center;
  }

  .meta {
    display: flex;
    flex-direction: column;
    gap: 2px;
  }

  .title {
    font-weight: 600;
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
  }

  .line {
    display: flex;
    justify-content: space-between;
    gap: 6px;
    font-size: 12px;
    color: var(--muted);
  }

  .warn {
    color: var(--warn);
  }
</style>
