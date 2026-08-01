<template>
  <main v-cloak :class="{ ready: session?.ready }" style="display: grid; gap: 1rem">
    <header v-if="session?.ready">
      <h1 v-text="title"></h1>
      <p>Operations from café 東京 λ 🚀 𝌆</p>
    </header>
    <p v-else-if="session?.pending">Preparing telemetry…</p>
    <p v-else>Please sign in.</p>
    <input
      v-model.trim="query"
      :placeholder="prompt"
      @keyup.enter.prevent="search()"
    />
    <ul v-show="results.length > 0">
      <li v-for="item in results" :key="item.id" @click="select(item)">
        <span v-html="item.highlightedLabel"></span>
      </li>
    </ul>
    <ReportPanel v-bind="panelProps" v-on:refresh.once="reload">
      <template #default="{ summary }"><strong>{{ summary }}</strong></template>
    </ReportPanel>
  </main>
</template>
