<script lang="ts">
  import type { Store } from "$lib/api";
  import { formatSize, relativeTime } from "$lib/format";
  import { t, type Key } from "$lib/i18n.svelte";
  import { app, type View } from "$lib/store.svelte";
  import Icon from "./Icon.svelte";

  const sources: Store[] = ["steam", "epic", "ubisoft", "gog", "emulator", "other"];

  const nav: { view: View; label: Key }[] = [
    { view: "library", label: "nav.library" },
    { view: "unidentified", label: "nav.unidentified" },
    { view: "backups", label: "nav.backups" },
    { view: "disk", label: "nav.disk" },
    { view: "settings", label: "nav.settings" },
  ];

  const counts = $derived<Record<View, number | null>>({
    library: app.scan ? app.games.length : null,
    unidentified: app.scan ? app.scan.unidentified.length : null,
    backups: app.backups.length,
    disk: null,
    settings: null,
  });
</script>

<aside>
  <div class="brand">
    <Icon name="logo" size={28} color="var(--accent)" />
    <span>SaveVault</span>
  </div>

  <nav aria-label="Navigation">
    {#each nav as item (item.view)}
      <button class="nav" class:active={app.view === item.view} onclick={() => (app.view = item.view)}>
        {t(item.label)}
        {#if item.view === "unidentified" && counts.unidentified}
          <span class="pill">{counts.unidentified}</span>
        {:else if item.view === "settings" && app.update}
          <span class="dot" aria-label={t("nav.updateAvailable")}></span>
        {:else if counts[item.view] != null}
          <span class="muted">{counts[item.view]}</span>
        {/if}
      </button>
    {/each}
  </nav>

  {#if app.view === "library"}
    <fieldset>
      <legend class="caps">{t("filter.source")}</legend>
      {#each sources as s (s)}
        <label><input type="checkbox" bind:checked={app.sources[s]} /> {t(`source.${s}`)}</label>
      {/each}
    </fieldset>

    <fieldset>
      <legend class="caps">{t("filter.state")}</legend>
      <label><input type="checkbox" bind:checked={app.showInstalled} /> {t("filter.installed")}</label>
      <label><input type="checkbox" bind:checked={app.showNotInstalled} /> {t("filter.notInstalled")}</label>
    </fieldset>

    {#if app.scan && app.scan.accounts.length > 0}
      <div class="group">
        <label for="account" class="caps">{t("filter.account")}</label>
        <select id="account" class="input" bind:value={app.account}>
          <option value="">{t("filter.allAccounts", { n: app.scan.accounts.length })}</option>
          {#each app.scan.accounts as acc (acc)}
            <option value={acc}>{acc}</option>
          {/each}
        </select>
      </div>
    {/if}
  {/if}

  {#if app.progress}
    {@const p = app.progress}
    {@const pct = p.total ? Math.round((p.done / p.total) * 100) : null}
    <div class="progress" role="status" aria-live="polite">
      <span class="small label">{t(`progress.${p.task}`, { label: p.label })}</span>
      <div
        class="track"
        role="progressbar"
        aria-valuemin="0"
        aria-valuemax="100"
        aria-valuenow={pct ?? undefined}
        aria-label={t(`progress.${p.task}`, { label: p.label })}
      >
        <div class="fill" class:indeterminate={pct == null} style:width={pct == null ? "35%" : `${pct}%`}></div>
      </div>
      {#if pct != null}<span class="muted small">{pct} %</span>{/if}
    </div>
  {/if}

  <div class="summary">
    <span class="muted small">{t("summary.detected")}</span>
    <span class="big">{formatSize(app.totalSize)}</span>
    <span class="muted small">
      {t("summary.lastScan", { when: app.scan ? relativeTime(app.scan.scannedAt) : t("summary.never") })}
    </span>
  </div>
</aside>

<style>
  aside {
    width: 224px;
    flex-shrink: 0;
    background: var(--sidebar);
    border-right: 1px solid var(--border);
    display: flex;
    flex-direction: column;
    padding: 20px 14px;
    gap: 24px;
    overflow-y: auto;
  }

  .brand {
    display: flex;
    align-items: center;
    gap: 10px;
    padding: 0 6px;
    font-family: var(--font-display);
    font-weight: 700;
    font-size: 19px;
    letter-spacing: -0.01em;
  }

  nav {
    display: flex;
    flex-direction: column;
    gap: 2px;
  }

  .nav {
    display: flex;
    align-items: center;
    justify-content: space-between;
    padding: 10px;
    border-radius: var(--radius);
    background: transparent;
    border: 0;
    color: var(--text-soft);
    text-align: left;
  }

  .nav:hover {
    background: #181c23;
  }

  .nav.active {
    background: var(--raised);
    color: var(--text);
    font-weight: 600;
  }

  .pill {
    background: var(--accent);
    color: var(--accent-ink);
    font-weight: 700;
    font-size: 12px;
    padding: 1px 7px;
    border-radius: 10px;
  }

  .dot {
    width: 8px;
    height: 8px;
    border-radius: 50%;
    background: var(--info);
  }

  fieldset,
  .group {
    border: 0;
    margin: 0;
    padding: 0 6px;
    display: flex;
    flex-direction: column;
    gap: 8px;
  }

  legend {
    padding: 0;
    margin-bottom: 8px;
  }

  fieldset label {
    display: flex;
    gap: 8px;
    align-items: center;
  }

  input[type="checkbox"] {
    accent-color: var(--accent);
    width: 15px;
    height: 15px;
  }

  .progress {
    margin-top: auto;
    padding: 12px;
    border-radius: var(--radius);
    background: #171b22;
    border: 1px solid var(--accent-border);
    display: flex;
    flex-direction: column;
    gap: 6px;
  }

  .label {
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }

  .track {
    height: 6px;
    background: #262b35;
    border-radius: 3px;
    overflow: hidden;
  }

  .fill {
    height: 100%;
    background: var(--accent);
    border-radius: 3px;
    transition: width 0.2s;
  }

  .fill.indeterminate {
    animation: slide 1.2s ease-in-out infinite;
  }

  @keyframes slide {
    from {
      transform: translateX(-100%);
    }
    to {
      transform: translateX(300%);
    }
  }

  .progress + .summary {
    margin-top: 0;
  }

  .summary {
    margin-top: auto;
    padding: 12px;
    border-radius: var(--radius);
    background: #171b22;
    display: flex;
    flex-direction: column;
    gap: 4px;
  }

  .small {
    font-size: 12px;
  }

  .big {
    font-family: var(--font-display);
    font-size: 20px;
    font-weight: 700;
  }
</style>
