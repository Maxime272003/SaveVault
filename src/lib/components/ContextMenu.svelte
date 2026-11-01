<script lang="ts" module>
  export interface MenuItem {
    label: string;
    shortcut?: string;
    danger?: boolean;
    disabled?: boolean;
    action: () => void;
  }
</script>

<script lang="ts">
  let { x, y, items, onclose }: { x: number; y: number; items: MenuItem[]; onclose: () => void } = $props();

  let menu: HTMLDivElement;
  let pos = $state({ left: 0, top: 0 });

  // Garde le menu dans la fenêtre et donne le focus au premier élément.
  $effect(() => {
    const r = menu.getBoundingClientRect();
    pos = {
      left: Math.min(x, window.innerWidth - r.width - 8),
      top: Math.min(y, window.innerHeight - r.height - 8),
    };
    menu.querySelector<HTMLButtonElement>("button:not(:disabled)")?.focus();
  });

  function onkeydown(e: KeyboardEvent) {
    const buttons = [...menu.querySelectorAll<HTMLButtonElement>("button:not(:disabled)")];
    const i = buttons.indexOf(document.activeElement as HTMLButtonElement);
    if (e.key === "Escape") onclose();
    else if (e.key === "ArrowDown") buttons[(i + 1) % buttons.length]?.focus();
    else if (e.key === "ArrowUp") buttons[(i - 1 + buttons.length) % buttons.length]?.focus();
    else return;
    e.preventDefault();
    e.stopPropagation();
  }
</script>

<svelte:window onblur={onclose} onresize={onclose} />

<!-- svelte-ignore a11y_no_static_element_interactions, a11y_click_events_have_key_events -->
<div class="backdrop" onclick={onclose} oncontextmenu={(e) => { e.preventDefault(); onclose(); }}></div>
<div class="menu" role="menu" tabindex="-1" bind:this={menu} style:left="{pos.left}px" style:top="{pos.top}px" {onkeydown}>
  {#each items as item (item.label)}
    <button
      role="menuitem"
      class:danger={item.danger}
      disabled={item.disabled}
      onclick={() => {
        onclose();
        item.action();
      }}
    >
      <span>{item.label}</span>
      {#if item.shortcut}<kbd>{item.shortcut}</kbd>{/if}
    </button>
  {/each}
</div>

<style>
  .backdrop {
    position: fixed;
    inset: 0;
    z-index: 20;
  }

  .menu {
    position: fixed;
    z-index: 21;
    min-width: 220px;
    background: #1b1f27;
    border: 1px solid var(--border-strong);
    border-radius: 10px;
    padding: 6px;
    box-shadow: 0 12px 32px rgb(0 0 0 / 0.5);
    display: flex;
    flex-direction: column;
  }

  button {
    display: flex;
    justify-content: space-between;
    align-items: center;
    gap: 16px;
    padding: 9px 10px;
    background: transparent;
    border: 0;
    border-radius: 6px;
    text-align: left;
  }

  button:hover:not(:disabled),
  button:focus-visible {
    background: var(--raised);
    outline: none;
  }

  button.danger {
    color: var(--danger);
  }

  kbd {
    font-family: var(--font-body);
    font-size: 11px;
    color: var(--muted);
  }
</style>
