<script lang="ts">
  import { app } from "$lib/store.svelte";
</script>

<div class="toasts" role="status" aria-live="polite">
  {#each app.toasts as toast (toast.id)}
    <div class="toast" class:error={toast.kind === "error"}>{toast.text}</div>
  {/each}
</div>

<style>
  .toasts {
    position: fixed;
    bottom: 20px;
    left: 50%;
    transform: translateX(-50%);
    display: flex;
    flex-direction: column;
    gap: 8px;
    align-items: center;
    z-index: 10;
    pointer-events: none;
  }

  .toast {
    background: #232831;
    border: 1px solid var(--border-strong);
    color: var(--text);
    padding: 10px 16px;
    border-radius: var(--radius);
    box-shadow: 0 8px 24px rgb(0 0 0 / 0.4);
    max-width: 560px;
  }

  .toast.error {
    background: var(--danger-bg);
    border-color: var(--danger-border);
    color: #f6c4be;
  }
</style>
