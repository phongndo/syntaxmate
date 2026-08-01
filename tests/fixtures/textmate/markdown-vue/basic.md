# Vue card example

The Markdown host introduces a café 東京 λ card used by the 🚀 𝌆 dashboard.

```vue title="WelcomeCard.vue"
<template>
  <article :class="{ featured: active }" @click="active = !active">
    <h2>{{ title }}</h2>
    <slot name="details">No details yet.</slot>
  </article>
</template>

<script setup lang="ts">
import { ref } from 'vue'
defineProps<{ title: string }>()
const active = ref(false)
</script>

<style scoped>
.featured { border-color: rebeccapurple; }
</style>
```

Back in Markdown, the component remains an illustrative, fully closed snippet.
