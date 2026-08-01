<template>
  <main :class="{ ready: tasks.length }">
    <!-- A compact café dashboard for 日本語 readers 🚀 𝌆 -->
    <h1>{{ title }}</h1>
    <input v-model.trim="draft" @keyup.enter="addTask" placeholder="New task" />
    <p v-if="tasks.length === 0">Nothing queued.</p>
    <ul v-else>
      <li v-for="task in tasks" :key="task.id">{{ task.label }}</li>
    </ul>
  </main>
</template>

<script setup lang="ts">
import { ref } from 'vue'

const title = 'Launch list'
const draft = ref('')
const tasks = ref([{ id: 1, label: 'Review telemetry' }])
function addTask() {
  if (draft.value) tasks.value.push({ id: Date.now(), label: draft.value })
  draft.value = ''
}
</script>

<style scoped>
.ready { border-inline-start: 0.25rem solid seagreen; }
</style>
