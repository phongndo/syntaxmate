<script module lang="ts">
  export type Mission = {
    id: number;
    name: string;
    region: string;
    status: 'ready' | 'delayed' | 'complete';
    progress: number;
    color: string;
    summary: string;
  };
  export const safeSummary = '<strong>Flight note:</strong> weather windows are advisory.';
</script>
<script lang="ts">
  import { browser } from '$app/environment';
  import { flip } from 'svelte/animate';
  import { cubicOut } from 'svelte/easing';
  import { fade, fly } from 'svelte/transition';
  import type { Snippet } from 'svelte';
  import MissionCard from './MissionCard.svelte';
  import StatusPill from './StatusPill.svelte';
  type Props = {
    title?: string;
    missions?: Mission[];
    children?: Snippet<[Mission]>;
  };
  let { title = 'Orbital mission desk · café 日本語 🚀 𝌆', missions = [], children }: Props = $props();
  let query = $state('');
  let selected = $state<Mission | null>(null);
  let compact = $state(false);
  let showComplete = $state(true);
  let viewportWidth = $state(0);
  let panel: HTMLElement;
  let headingTag: 'h1' | 'h2' = 'h1';
  let featuredPromise = $state(loadFeatured());
  let filtered = $derived(missions.filter((mission) =>
    mission.name.toLocaleLowerCase().includes(query.toLocaleLowerCase()) &&
    (showComplete || mission.status !== 'complete')
  ));
  let average = $derived.by(() => {
    if (filtered.length === 0) return 0;
    return filtered.reduce((sum, item) => sum + item.progress, 0) / filtered.length;
  });

  $effect(() => {
    if (!browser) return;
    document.title = `${title} · ${filtered.length}`;
  });

  async function loadFeatured(): Promise<Mission> {
    await Promise.resolve();
    const mission = missions.find((item) => item.status === 'ready');
    if (!mission) throw new Error('No launch-ready mission');
    return mission;
  }

  function choose(mission: Mission) { selected = mission; }

  function handleShortcut(event: KeyboardEvent) {
    if (event.key === 'Escape') selected = null;
    if (event.key === '/' && panel) panel.querySelector('input')?.focus();
  }

  function tooltip(node: HTMLElement, text: string) {
    node.setAttribute('aria-label', text);
    return {
      update(next: string) { node.setAttribute('aria-label', next); },
      destroy() { node.removeAttribute('aria-label'); }
    };
  }

  function measure(node: HTMLElement) {
    const observer = new ResizeObserver(([entry]) => compact = entry.contentRect.width < 560);
    observer.observe(node);
    return () => observer.disconnect();
  }
</script>

<svelte:options customElement="mission-board" />
<svelte:head>
  <meta name="description" content="A live mission planning dashboard" />
</svelte:head>
<svelte:window bind:innerWidth={viewportWidth} on:keydown={handleShortcut} />
<svelte:body class:has-selection={selected !== null} />

{#snippet statusLabel(mission: Mission)}
  <span class="status" data-state={mission.status}>
    {mission.status === 'ready' ? 'Launch ready' : mission.status}
  </span>
{/snippet}

<!-- The board remains useful before live telemetry connects. -->
<section class:compact class:wide={viewportWidth > 1200} bind:this={panel}
  {@attach measure} aria-labelledby="board-title">
  <header in:fly={{ y: -12, duration: 240, easing: cubicOut }}>
    <div>
      <svelte:element this={headingTag} id="board-title">{title}</svelte:element>
      <p>{filtered.length} missions · {average.toFixed(0)}% average progress</p>
    </div>
    <button class:active={compact} style:opacity={compact ? 1 : 0.72}
      use:tooltip="Toggle compact cards" on:click={() => compact = !compact}>
      {compact ? 'Comfortable view' : 'Compact view'}
    </button>
  </header>

  <form on:submit|preventDefault={() => featuredPromise = loadFeatured()}>
    <label for="mission-search">Search missions</label>
    <input id="mission-search" type="search" placeholder="Name or callsign" bind:value={query} />
    <label class="check">
      <input type="checkbox" bind:checked={showComplete} />
      Include completed flights
    </label>
    <button type="submit">Refresh featured mission</button>
  </form>

  {#await featuredPromise}
    <p class="notice" transition:fade>Contacting flight control…</p>
  {:then featured}
    <aside class="featured" style:--accent={featured.color} transition:fly|local={{ x: 16 }}>
      <strong>Featured:</strong> {featured.name}
      {@render statusLabel(featured)}
    </aside>
  {:catch error}
    <p class="error" role="alert" transition:fade>{error.message}</p>
  {/await}

  {#if filtered.length > 0}
    <ul class="missions" class:dense={compact}>
      {#each filtered as mission, index (mission.id)}
        {@const percent = Math.min(100, Math.max(0, mission.progress))}
        <li animate:flip={{ duration: 220 }} class:selected={selected?.id === mission.id}
          style:--accent={mission.color}>
          <button type="button" on:click|stopPropagation={() => choose(mission)}
            use:tooltip={`Open ${mission.name}`}>
            <span class="index">{String(index + 1).padStart(2, '0')}</span>
            <span>
              <strong>{mission.name}</strong>
              <small>{mission.region} · {percent}%</small>
            </span>
            {@render statusLabel(mission)}
          </button>
          <progress max="100" value={percent}>{percent}%</progress>
          {#if children}
            {@render children(mission)}
          {:else if mission.summary}
            <p>{mission.summary}</p>
          {:else}
            <p class="muted">No flight notes yet.</p>
          {/if}
        </li>
      {:else}
        <li class="empty">No missions match “{query}”.</li>
      {/each}
    </ul>
  {:else}
    <p class="empty" out:fade>No matching missions. Try a shorter callsign.</p>
  {/if}

  {#key selected?.id}
    {#if selected}
      <div class="scrim" role="presentation" on:click={() => selected = null}>
        <article class="details" role="dialog" aria-modal="true"
          aria-label={`Mission ${selected.name}`} transition:fly={{ y: 20, duration: 180 }}
          on:click|stopPropagation>
          <button class="close" on:click={() => selected = null} aria-label="Close">×</button>
          <MissionCard mission={selected}>
            <StatusPill status={selected.status} />
            <p>Flight region: {selected.region}</p>
          </MissionCard>
          <svelte:component this={StatusPill} status={selected.status} />
          <div class="raw-note">{@html safeSummary}</div>
        </article>
      </div>
    {/if}
  {/key}
</section>

<style lang="scss">
  $ink: #172033;
  $surface: #f7f8fc;

  :global(body) { margin: 0; background: $surface; color: $ink; }
  :global(body.has-selection) { overflow: hidden; }

  section {
    max-width: 70rem;
    margin: 0 auto;
    padding: clamp(1rem, 3vw, 2.5rem);

    &.compact .missions button { padding-block: 0.45rem; }
    &.wide { max-width: 82rem; }
  }

  header, form, .missions button {
    display: flex;
    align-items: center;
    gap: 0.8rem;
  }

  header { justify-content: space-between; }
  form { flex-wrap: wrap; margin: 1.5rem 0; }
  input[type='search'] { flex: 1 1 18rem; padding: 0.65rem; }
  .check { display: inline-flex; gap: 0.4rem; }
  .active, .selected { outline: 2px solid #6d4aff; }

  .missions { display: grid; gap: 0.8rem; padding: 0; list-style: none; }
  .missions li { border-left: 0.3rem solid var(--accent); background: white; }
  .missions button { width: 100%; border: 0; background: transparent; text-align: left; }
  .missions button > span:nth-child(2) { flex: 1; }
  .missions small, .muted { display: block; color: #647089; }
  progress { width: 100%; accent-color: var(--accent); }

  .status { padding: 0.2rem 0.5rem; border-radius: 999px; background: #e9ecf4; }
  .status[data-state='ready'] { background: #c9f4dc; }
  .featured { border-inline-start: 0.3rem solid var(--accent); padding: 0.8rem; }
  .error { color: #a32135; }
  .empty { padding: 2rem; text-align: center; }

  .scrim { position: fixed; inset: 0; display: grid; place-items: center; background: #101522aa; }
  .details { position: relative; width: min(32rem, 90vw); padding: 1.5rem; background: white; }
  .close { position: absolute; inset: 0.5rem 0.5rem auto auto; }
  .raw-note :global(strong) { color: #5b3fd1; }

  @media (max-width: 40rem) {
    header { align-items: flex-start; flex-direction: column; }
    .status { font-size: 0.75rem; }
  }
</style>
