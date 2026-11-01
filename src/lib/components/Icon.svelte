<script lang="ts">
  type Name = "logo" | "search" | "refresh" | "cloud" | "folder" | "trash" | "check" | "alert" | "x" | "download" | "link";

  let { name, size = 16, color = "currentColor", label }: { name: Name; size?: number; color?: string; label?: string } =
    $props();
</script>

<svg
  width={size}
  height={size}
  viewBox="0 0 24 24"
  fill="none"
  stroke={color}
  stroke-width={name === "logo" ? 1.8 : 2}
  stroke-linecap="round"
  stroke-linejoin="round"
  role={label ? "img" : undefined}
  aria-label={label}
  aria-hidden={label ? undefined : "true"}
  style="flex-shrink: 0"
>
  {#if name === "logo"}
    <rect x="3" y="4" width="18" height="16" rx="2" /><circle cx="12" cy="12" r="3.5" /><path
      d="M12 8.5V7M15.5 12H17M12 15.5V17M8.5 12H7"
    />
  {:else if name === "search"}
    <circle cx="11" cy="11" r="7" /><path d="m20 20-3.5-3.5" />
  {:else if name === "refresh"}
    <path d="M20 11a8 8 0 1 0-2.3 5.7" /><path d="M20 4v7h-7" />
  {:else if name === "cloud"}
    <path d="M7 18h10a4 4 0 0 0 .5-8 6 6 0 0 0-11.4 1.5A3.3 3.3 0 0 0 7 18z" />
  {:else if name === "folder"}
    <path d="M3 7a2 2 0 0 1 2-2h4l2 2h8a2 2 0 0 1 2 2v8a2 2 0 0 1-2 2H5a2 2 0 0 1-2-2z" />
  {:else if name === "trash"}
    <path d="M4 7h16M9 7V4h6v3M6 7l1 13h10l1-13" />
  {:else if name === "check"}
    <path d="m5 12 5 5 9-10" />
  {:else if name === "alert"}
    <path d="M12 9v4M12 17h.01" /><path d="M10.3 3.9 2.4 18a2 2 0 0 0 1.7 3h15.8a2 2 0 0 0 1.7-3L13.7 3.9a2 2 0 0 0-3.4 0z" />
  {:else if name === "x"}
    <path d="M18 6 6 18M6 6l12 12" />
  {:else if name === "download"}
    <path d="M12 4v11M7 10l5 5 5-5M5 20h14" />
  {:else if name === "link"}
    <path d="M10 14a4 4 0 0 0 5.7 0l3-3a4 4 0 0 0-5.7-5.7l-1 1" /><path
      d="M14 10a4 4 0 0 0-5.7 0l-3 3a4 4 0 0 0 5.7 5.7l1-1"
    />
  {/if}
</svg>
