<template>
  <article class="mission-card">
    <h1>{{ title }}</h1>
    <p>Weather from café 東京 λ 🚀 𝌆</p>
  </article>
</template>

<script setup lang="ts">
const title = 'Orbital forecast'
const mode = 'night'
const fallback = '#dbeafe'
const theme = { colors: { night: { text: '#eff6ff' }, accent: '#38bdf8' }, shadow: { blur: '1rem' } }
const resolveShadow = (blur: string, strength: number) => `0 0 ${blur} rgb(56 189 248 / ${strength})`
</script>

<style scoped>
.mission-card {
  /* v-bind(theme.colors.commentOnly) must remain a CSS comment. */
  color: v-bind(theme.colors[mode]?.text ?? fallback);
  border-color: v-bind('theme.colors.accent');
  border-radius: v-bind("theme.shadow.blur");
  box-shadow: v-bind(resolveShadow(theme.shadow.blur, 0.35));
}
</style>
