<script lang="ts">
  import type { Snippet } from "svelte";
  import { t } from "$lib/i18n.svelte";
  import Icon from "./Icon.svelte";

  let {
    title,
    icon,
    tone = "neutral",
    confirmLabel,
    confirmDisabled = false,
    onconfirm,
    onclose,
    children,
  }: {
    title: string;
    icon?: "trash" | "download" | "folder";
    tone?: "neutral" | "danger" | "accent";
    confirmLabel?: string;
    confirmDisabled?: boolean;
    onconfirm?: () => Promise<void> | void;
    onclose: () => void;
    children: Snippet;
  } = $props();

  let dialog: HTMLDialogElement;
  let busy = $state(false);

  $effect(() => {
    dialog.showModal();
  });

  async function confirm() {
    if (!onconfirm) return;
    busy = true;
    try {
      await onconfirm();
      onclose();
    } finally {
      busy = false;
    }
  }
</script>

<dialog
  bind:this={dialog}
  aria-labelledby="dlg-title"
  oncancel={(e) => {
    e.preventDefault();
    if (!busy) onclose();
  }}
>
  <div class="head">
    {#if icon}
      <span class="badge {tone}">
        <Icon name={icon} size={20} />
      </span>
    {/if}
    <h2 id="dlg-title">{title}</h2>
  </div>

  <div class="body">
    {@render children()}
  </div>

  <div class="actions">
    <button class="btn tall" onclick={onclose} disabled={busy}>{t("confirm.cancel")}</button>
    {#if confirmLabel}
      <button
        class="btn tall"
        class:danger-solid={tone === "danger"}
        class:primary={tone !== "danger"}
        onclick={confirm}
        disabled={busy || confirmDisabled}
      >
        {confirmLabel}
      </button>
    {/if}
  </div>
</dialog>

<style>
  dialog {
    width: min(560px, calc(100vw - 32px));
    background: #161920;
    color: var(--text);
    border: 1px solid var(--border-strong);
    border-radius: 14px;
    padding: 28px;
    display: flex;
    flex-direction: column;
    gap: 16px;
  }

  dialog:not([open]) {
    display: none;
  }

  dialog::backdrop {
    background: rgb(5 6 8 / 0.7);
  }

  .head {
    display: flex;
    align-items: center;
    gap: 12px;
  }

  h2 {
    margin: 0;
    font-family: var(--font-display);
    font-size: 21px;
    font-weight: 700;
  }

  .badge {
    width: 40px;
    height: 40px;
    border-radius: 10px;
    display: flex;
    align-items: center;
    justify-content: center;
    flex-shrink: 0;
    background: var(--raised);
  }

  .badge.danger {
    background: var(--danger-bg);
    color: var(--danger);
  }

  .badge.accent {
    background: #3a2d14;
    color: var(--accent);
  }

  .body {
    display: flex;
    flex-direction: column;
    gap: 14px;
    line-height: 1.45;
    max-height: 60vh;
    overflow-y: auto;
  }

  .actions {
    display: flex;
    justify-content: flex-end;
    gap: 10px;
    margin-top: 4px;
  }
</style>
