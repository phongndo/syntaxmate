# Mission board component guide

This Markdown page documents a Vue single-file component for café 東京 λ crews.
The unusual astral symbols 🚀 and 𝌆 deliberately pass through prose and templates.

## Complete board

The attributed fence is the fragment root; template, TypeScript, and scoped style
regions remain nested inside it until the matching Markdown delimiter.

```vue title="MissionBoard.vue" {data-preview=interactive}
<template>
  <BoardShell
    v-model:page.number="page"
    :class="[$style.panel, { [$style.offline]: !online }]"
    :data-board-id="boardId"
    v-bind="shellAttributes"
    @refresh="refreshBoard"
  >
    <template #toolbar="{ refresh }">
      <form class="toolbar" @submit.prevent="addMission">
        <label for="mission-search">Search missions</label>
        <input
          id="mission-search"
          v-model.trim="query"
          :[activeAttribute]="activeValue"
          @keyup.esc="query = ''"
        />
        <select v-model="filter" aria-label="Mission filter">
          <option value="all">All</option>
          <option value="ready">Ready</option>
          <option value="held">Held</option>
        </select>
        <button type="button" :disabled="loading" @click.stop="refresh()">
          Reload
        </button>
        <button type="submit">Add</button>
      </form>
    </template>

    <!-- Balanced Vue comments and interpolation belong to the embedded grammar. -->
    <header>
      <h1>{{ boardTitle }}</h1>
      <p v-if="loading">Synchronizing…</p>
      <p v-else-if="error" role="alert">{{ error.message }}</p>
      <p v-else>{{ statusText }}</p>
    </header>

    <TransitionGroup name="mission" tag="ul" class="mission-list">
      <li
        v-for="(mission, index) in visibleMissions"
        :key="mission.id"
        :class="{ selected: mission.id === selectedId }"
        @click="selectedId = mission.id"
      >
        <strong>{{ index + 1 }}. {{ mission.name }}</strong>
        <span>{{ mission.destination ?? 'unassigned' }}</span>
        <button @click.stop="removeMission(mission.id)">Remove</button>
      </li>
    </TransitionGroup>

    <EmptyState v-if="visibleMissions.length === 0" @clear="resetFilters">
      <template #icon><span aria-hidden="true">◎</span></template>
      No matching mission for “{{ query }}”.
    </EmptyState>

    <KeepAlive :max="2">
      <component :is="activePanel" :mission-id="selectedId" />
    </KeepAlive>

    <Suspense>
      <AsyncSummary :missions="missions" />
      <template #fallback><p class="skeleton">Preparing summary…</p></template>
    </Suspense>

    <details>
      <summary>Raw diagnostics</summary>
      <pre v-pre>{{ untouched }} café 東京 λ 🚀 𝌆</pre>
      <div v-html="trustedIntroduction"></div>
    </details>

    <Teleport to="#modals" :disabled="selectedId === null">
      <MissionDialog
        v-if="selectedMission"
        :mission="selectedMission"
        @close="selectedId = null"
      />
    </Teleport>

    <template #footer>
      <small v-once>Board {{ version }} · {{ localeLabel }}</small>
      <slot name="legal" :updated-at="updatedAt">Locally cached.</slot>
    </template>
  </BoardShell>
</template>

<script lang="ts">
export const version = '3.1.0'
export default { inheritAttrs: false }
</script>

<script setup lang="ts">
import { computed, defineAsyncComponent, onMounted, reactive, ref, watch } from 'vue'
import BoardShell from './BoardShell.vue'
import EmptyState from './EmptyState.vue'
import MissionDialog from './MissionDialog.vue'

interface Mission {
  id: number
  name: string
  destination: string | null
  state: 'ready' | 'held'
}

type Filter = 'all' | Mission['state']

const props = withDefaults(defineProps<{
  boardId: string
  endpoint?: string
  initial?: Mission[]
}>(), {
  endpoint: '/api/missions',
  initial: () => [],
})

const emit = defineEmits<{
  saved: [missions: Mission[]]
  failed: [cause: Error]
}>()
const selectedId = defineModel<number | null>('selectedId', { default: null })
const AsyncSummary = defineAsyncComponent(() => import('./AsyncSummary.vue'))
const missions = reactive<Mission[]>(props.initial.length ? props.initial : [
  { id: 1, name: 'Café relay 🚀', destination: '東京', state: 'ready' },
  { id: 2, name: 'Lambda survey 𝌆', destination: null, state: 'held' },
])
const query = ref('')
const filter = ref<Filter>('all')
const page = ref(1)
const loading = ref(false)
const online = ref(true)
const error = ref<Error | null>(null)
const activePanel = ref('MissionInspector')
const activeAttribute = ref<'title' | 'aria-label'>('title')
const updatedAt = ref(new Date())
const localeLabel = 'café · 東京 · λ · 🚀 · 𝌆'
const trustedIntroduction = '<em>Operator-authored fixture content</em>'
const shellAttributes = { role: 'main', 'data-density': 'comfortable' }

const boardTitle = computed(() => `Mission board ${props.boardId}`)
const activeValue = computed(() => `Search ${missions.length} missions`)
const statusText = computed(() => `${missions.filter(({ state }) => state === 'ready').length} ready`)
const visibleMissions = computed(() => missions.filter((mission) => {
  const textMatches = mission.name.toLowerCase().includes(query.value.toLowerCase())
  const stateMatches = filter.value === 'all' || mission.state === filter.value
  return textMatches && stateMatches
}))
const selectedMission = computed(() =>
  missions.find(({ id }) => id === selectedId.value) ?? null,
)

function addMission(): void {
  const name = query.value.trim()
  if (!name) return
  missions.push({ id: Date.now(), name, destination: null, state: 'held' })
  query.value = ''
}

function removeMission(id: number): void {
  const index = missions.findIndex((mission) => mission.id === id)
  if (index >= 0) missions.splice(index, 1)
}

function resetFilters(): void {
  filter.value = 'all'
  query.value = ''
}

async function refreshBoard(): Promise<void> {
  loading.value = true
  error.value = null
  try {
    const response = await fetch(props.endpoint)
    if (!response.ok) throw new Error(`Request failed: ${response.status}`)
    missions.splice(0, missions.length, ...await response.json() as Mission[])
    emit('saved', [...missions])
  } catch (cause: unknown) {
    error.value = cause instanceof Error ? cause : new Error(String(cause))
    emit('failed', error.value)
  } finally {
    loading.value = false
  }
}

watch(filter, () => { page.value = 1 })
onMounted(() => { online.value = navigator.onLine })
defineExpose({ refresh: refreshBoard })
</script>

<style scoped lang="scss">
$surface: #171923;
.toolbar { display: flex; gap: 0.5rem; }
.mission-list {
  background: $surface;
  & > .selected { color: v-bind('props.boardId'); }
}
.mission-enter-active,
.mission-leave-active { transition: opacity 180ms ease; }
.mission-enter-from,
.mission-leave-to { opacity: 0; }
:deep(.empty-state) { padding: 1rem; }
@media (prefers-reduced-motion: reduce) {
  .mission-enter-active { transition: none; }
}
</style>

<style module>
.panel { min-block-size: 20rem; }
.offline { opacity: 0.7; }
</style>
```

## Compact variant

The tilde form and case-insensitive language name cover a second fence opening.

~~~VUE preview=compact
<template>
  <output :data-count="items.length">
    <slot>{{ items.join(' · ') }}</slot>
  </output>
</template>
<script setup>
defineProps({ items: { type: Array, default: () => [] } })
</script>
~~~

After both examples, Markdown list and link parsing can resume:

- Verify the preview.
- Read the [component contract](https://example.test/components/mission-board).
