<script lang="ts">
  import { formatSize } from "$lib/format";
  import { t } from "$lib/i18n.svelte";
  import { app } from "$lib/store.svelte";
  import Cover from "./Cover.svelte";
  import GameDetail from "./GameDetail.svelte";
  import Icon from "./Icon.svelte";
</script>

<div class="library">
  <main>
    <div class="toolbar">
      <label for="q" class="sr-only">{t("library.search")}</label>
      <div class="search">
        <Icon name="search" color="var(--muted)" />
        <input id="q" placeholder={t("library.search")} bind:value={app.search} />
      </div>
      <select class="input" aria-label={t("library.sort")} bind:value={app.sort}>
        <option value="modified">{t("library.sortModified")}</option>
        <option value="name">{t("library.sortName")}</option>
        <option value="size">{t("library.sortSize")}</option>
      </select>
      <button class="btn" onclick={() => app.rescan()} disabled={app.scanning}>
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
        <div class="grid">
          {#each app.filtered as game (game.id)}
            <button
              class="card"
              class:selected={game.id === app.selectedId}
              onclick={() => (app.selectedId = game.id)}
              aria-label={game.title}
              aria-pressed={game.id === app.selectedId}
            >
              <div class="art">
                <Cover appId={game.steamAppId} title={game.title} />
                <div class="badges">
                  <span class="source">{t(`source.${game.source}`)}</span>
                  {#if game.cloudSteam}
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
    gap: 12px;
    padding: 18px 24px;
    border-bottom: 1px solid #1e232b;
  }

  .search {
    flex: 1;
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

  .card.selected .art {
    outline-color: var(--accent);
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
