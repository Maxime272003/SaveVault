<script lang="ts">
  import { formatSize } from "$lib/format";
  import { t } from "$lib/i18n.svelte";
  import { app } from "$lib/store.svelte";

  const TOP = 12;

  let asTable = $state(false);

  const largest = $derived([...app.games].sort((a, b) => b.size - a.size).slice(0, TOP));
  const max = $derived(largest[0]?.size || 1);
  const leftovers = $derived(app.games.filter((g) => !g.installed).sort((a, b) => b.size - a.size));
  const leftoverSize = $derived(leftovers.reduce((s, g) => s + g.size, 0));
  const backupTotal = $derived(app.backups.reduce((s, b) => s + b.size, 0));
  const backupsByGame = $derived(
    [
      ...app.backups
        .reduce((m, b) => {
          const e = m.get(b.game) ?? { game: b.game, size: 0, count: 0 };
          e.size += b.size;
          e.count++;
          return m.set(b.game, e);
        }, new Map<string, { game: string; size: number; count: number }>())
        .values(),
    ]
      .sort((a, b) => b.size - a.size)
      .slice(0, 5),
  );

  function show(id: string) {
    app.selectedId = id;
    app.view = "library";
  }

  function clean(ids: string[]) {
    if (app.requireBackupDir()) app.dialog = { kind: "clean", gameIds: ids };
  }
</script>

<div class="page">
  <h1>{t("disk.title")}</h1>

  <div class="tiles">
    <div class="tile">
      <span class="caps">{t("disk.saves")}</span>
      <span class="value">{formatSize(app.totalSize)}</span>
      <span class="muted small">{t("disk.games", { n: app.games.length })}</span>
    </div>
    <div class="tile">
      <span class="caps">{t("disk.uninstalled")}</span>
      <span class="value">{formatSize(leftoverSize)}</span>
      <span class="muted small">{t("disk.games", { n: leftovers.length })}</span>
    </div>
    <div class="tile">
      <span class="caps">{t("disk.backups")}</span>
      <span class="value">{formatSize(backupTotal)}</span>
      <span class="muted small">{t("settings.backupStats", { n: app.backups.length })}</span>
    </div>
  </div>

  <section>
    <div class="head">
      <h2>{t("disk.largest")}</h2>
      <button class="btn small ghost" onclick={() => (asTable = !asTable)} aria-pressed={asTable}>
        {asTable ? t("disk.chart") : t("disk.table")}
      </button>
    </div>
    {#if asTable}
      <table>
        <thead>
          <tr><th>{t("disk.game")}</th><th>{t("filter.state")}</th><th class="num">{t("disk.size")}</th></tr>
        </thead>
        <tbody>
          {#each largest as g (g.id)}
            <tr>
              <td><button class="name" onclick={() => show(g.id)}>{g.title}</button></td>
              <td class="muted">{g.installed ? t("filter.installed") : t("filter.notInstalled")}</td>
              <td class="num">{formatSize(g.size)}</td>
            </tr>
          {/each}
        </tbody>
      </table>
    {:else}
      <ol class="bars">
        {#each largest as g (g.id)}
          <li title="{g.title} · {formatSize(g.size)} · {t('disk.files', { n: g.fileCount })}">
            <button class="name" onclick={() => show(g.id)}>{g.title}</button>
            <div class="track">
              <div class="bar" style:width="{Math.max(0.5, (g.size / max) * 100)}%"></div>
            </div>
            <span class="num">{formatSize(g.size)}</span>
          </li>
        {/each}
      </ol>
    {/if}
  </section>

  <section>
    <div class="head">
      <h2>{t("disk.leftovers")}</h2>
      {#if leftovers.length}
        <button class="btn small danger" onclick={() => clean(leftovers.map((g) => g.id))}>
          {t("disk.cleanAll", { size: formatSize(leftoverSize) })}
        </button>
      {/if}
    </div>
    {#if leftovers.length}
      <p class="muted small hint">{t("disk.leftoversHint")}</p>
      <ul class="rows">
        {#each leftovers as g (g.id)}
          <li>
            <button class="name" onclick={() => show(g.id)}>{g.title}</button>
            <span class="muted">{formatSize(g.size)}</span>
            <button class="btn small danger" onclick={() => app.requireBackupDir() && (app.dialog = { kind: "delete", gameId: g.id })}>
              {t("backup.delete")}
            </button>
          </li>
        {/each}
      </ul>
    {:else}
      <p class="muted">{t("disk.none")}</p>
    {/if}
  </section>

  {#if backupsByGame.length}
    <section>
      <h2>{t("disk.heaviestBackups")}</h2>
      <ul class="rows">
        {#each backupsByGame as b (b.game)}
          <li>
            <span class="name static">{b.game}</span>
            <span class="muted">{t("settings.backupStats", { n: b.count })}</span>
            <span>{formatSize(b.size)}</span>
          </li>
        {/each}
      </ul>
    </section>
  {/if}
</div>

<style>
  .page {
    flex: 1;
    min-width: 0;
    overflow-y: auto;
    padding: 32px 40px;
    display: flex;
    flex-direction: column;
    gap: 28px;
  }

  h1 {
    margin: 0;
    font-family: var(--font-display);
    font-size: 28px;
  }

  h2 {
    margin: 0;
    font-size: 16px;
    font-weight: 600;
  }

  .tiles {
    display: grid;
    grid-template-columns: repeat(auto-fit, minmax(200px, 1fr));
    gap: 16px;
  }

  .tile {
    background: var(--panel);
    border: 1px solid var(--border);
    border-radius: 12px;
    padding: 18px 20px;
    display: flex;
    flex-direction: column;
    gap: 6px;
  }

  .value {
    font-family: var(--font-display);
    font-size: 28px;
    font-weight: 700;
  }

  .small {
    font-size: 12px;
  }

  section {
    display: flex;
    flex-direction: column;
    gap: 12px;
  }

  .head {
    display: flex;
    justify-content: space-between;
    align-items: center;
    gap: 12px;
  }

  .hint {
    margin: 0;
  }

  .bars {
    list-style: none;
    margin: 0;
    padding: 0;
    display: flex;
    flex-direction: column;
    gap: 6px;
  }

  .bars li {
    display: grid;
    grid-template-columns: minmax(140px, 260px) minmax(0, 1fr) 80px;
    align-items: center;
    gap: 14px;
    padding: 4px 0;
  }

  .track {
    height: 10px;
  }

  .bar {
    height: 100%;
    background: var(--accent);
    border-radius: 0 4px 4px 0;
  }

  .bars li:hover .bar {
    background: #f7c46a;
  }

  .num {
    text-align: right;
    font-variant-numeric: tabular-nums;
    color: var(--text-soft);
  }

  .name {
    background: transparent;
    border: 0;
    padding: 0;
    text-align: left;
    color: var(--text);
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
    min-width: 0;
  }

  button.name:hover {
    color: var(--accent);
  }

  table {
    border-collapse: collapse;
    width: 100%;
    max-width: 720px;
  }

  th,
  td {
    text-align: left;
    padding: 8px 10px;
    border-bottom: 1px solid var(--divider);
  }

  th {
    font-size: 12px;
    color: var(--muted);
    font-weight: 600;
  }

  th.num,
  td.num {
    text-align: right;
  }

  .rows {
    list-style: none;
    margin: 0;
    padding: 0;
    display: flex;
    flex-direction: column;
  }

  .rows li {
    display: grid;
    grid-template-columns: minmax(0, 1fr) auto auto;
    gap: 16px;
    align-items: center;
    padding: 8px 4px;
    border-bottom: 1px solid var(--divider);
  }

  .static {
    font-weight: 500;
  }
</style>
