<script>
  export let name = 'traveler';
  let count = 0;
  $: doubled = count * 2;
</script>

<svelte:head>
  <title>Hello {name}</title>
</svelte:head>

<main class:busy={count > 3}>
  <h1>Welcome, {name}!</h1>
  {#if count === 0}
    <p>Choose a first mission: café, 日本語, 🚀 𝌆.</p>
  {:else}
    <p>You launched {count} times ({doubled} engine pulses).</p>
  {/if}
  <button on:click={() => count += 1}>Launch</button>
</main>

<style>
  main { max-width: 32rem; margin: 2rem auto; }
  .busy { border-left: 0.25rem solid rebeccapurple; }
  button:hover { transform: translateY(-1px); }
</style>
