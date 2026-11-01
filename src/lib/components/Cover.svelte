<script lang="ts">
  import { loadCover, type CoverImage } from "$lib/covers";
  import { titleColor } from "$lib/format";

  let {
    appId,
    title,
    showTitle = true,
    position = "center",
  }: { appId: number | null; title: string; showTitle?: boolean; position?: string } = $props();

  let el: HTMLDivElement;
  let image = $state<CoverImage | null>(null);
  let failed = $state(false);

  $effect(() => {
    const id = appId;
    const name = title;
    image = null;
    failed = false;
    let cancelled = false;
    const io = new IntersectionObserver(
      (entries) => {
        if (!entries.some((e) => e.isIntersecting)) return;
        io.disconnect();
        loadCover(id, name).then((c) => {
          if (!cancelled) image = c;
        });
      },
      { rootMargin: "300px" },
    );
    io.observe(el);
    return () => {
      cancelled = true;
      io.disconnect();
    };
  });
</script>

<div class="cover" bind:this={el} style:background={titleColor(title)}>
  {#if image && !failed}
    <img
      src={image.src}
      alt=""
      style:object-position={image.portrait ? position : "center"}
      onerror={() => (failed = true)}
    />
  {:else if showTitle}
    <span class="name">{title}</span>
  {/if}
</div>

<style>
  .cover {
    position: absolute;
    inset: 0;
    display: flex;
    align-items: flex-end;
    padding: 12px;
    overflow: hidden;
  }

  img {
    position: absolute;
    inset: 0;
    width: 100%;
    height: 100%;
    object-fit: cover;
  }

  .name {
    font-family: var(--font-display);
    font-weight: 700;
    font-size: 17px;
    line-height: 1.15;
    overflow-wrap: anywhere;
  }
</style>
