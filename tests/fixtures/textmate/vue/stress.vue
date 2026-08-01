<template>
  <BoardShell v-model:page.number="page" :class="[$style.panel, { [$style.active]: online }]"
    :title="boardTitle" :data-board.camel="boardId" v-bind="shellAttributes"
    @refresh="refreshBoard">
    <template #toolbar="{ refresh }">
      <form class="toolbar" @submit.prevent="addTask">
        <label for="task-search">Search</label>
        <input id="task-search" v-model.trim="query" :[activeAttribute]="activeValue"
          @keyup.esc="query = ''" @focus="lastAction = 'search focused'" />
        <select v-model="filter" aria-label="Task filter">
          <option value="all">All tasks</option>
          <option value="open">Open</option>
          <option value="done">Done</option>
        </select>
        <button type="button" :disabled @click.stop="refresh()">Reload</button>
        <button type="submit" @[secondaryEvent].once="lastAction = 'Shortcut used'">Add task</button>
      </form>
    </template>
    <!-- @fixture { "kind": "vue-sfc", "balanced": true } -->
    <header class="board__header">
      <h1>{{ boardTitle }}</h1>
      <p v-if="loading">Synchronizing with {{ endpoint }}…</p>
      <p v-else-if="error" role="alert">{{ error.message }}</p>
      <p v-else>{{ statusText }}</p>
      <span class="sr-only" :text-content.prop="statusText"></span>
    </header>
    <section v-show="online" class="board__content" aria-live="polite">
      <TransitionGroup name="task" tag="ul" class="task-list">
        <TaskRow v-for="(task, index) in visibleTasks" :key="task.id"
          v-model:complete="task.done" :task="task" :position="index + 1"
          :class="{ overdue: isOverdue(task) }" @remove="removeTask(task.id)"
          @rename="task.title = $event">
          <template #meta="{ dueLabel }">
            <time :datetime="task.due ?? undefined">{{ dueLabel }}</time>
          </template>
        </TaskRow>
      </TransitionGroup>
      <EmptyState v-if="visibleTasks.length === 0" :query="query" @clear="resetFilters">
        <template #icon><span aria-hidden="true">◎</span></template>
      </EmptyState>
      <p v-else class="summary">Showing {{ visibleTasks.length }} of {{ tasks.length }}</p>
    </section>
    <Transition name="notice" mode="out-in">
      <aside v-if="lastAction" :key="lastAction" class="notice"
        @click.self="lastAction = ''">{{ lastAction }}</aside>
    </Transition>
    <KeepAlive :max="2">
      <component :is="activePanel" :tasks="tasks"
        @select="selectedId = $event; showDialog = true" />
    </KeepAlive>
    <Suspense>
      <AsyncSummary :tasks="tasks" />
      <template #fallback><p class="skeleton">Preparing summary…</p></template>
    </Suspense>
    <template #[activeSlot]="slotProps">
      <strong>{{ slotProps.message }}</strong>
    </template>
    <details class="developer-note">
      <summary>Template diagnostics</summary>
      <pre v-pre>Raw Vue text: {{ untouched }} café 日本語 🚀 𝌆</pre>
      <code>{{ JSON.stringify({ page, filter }, null, 2) }}</code>
      <div v-html="introHtml"></div>
    </details>

    <Teleport to="#modals" :disabled="!showDialog">
      <ConfirmDialog v-if="showDialog" :task="selectedTask"
        @confirm="confirmRemoval" @cancel="showDialog = false" />
    </Teleport>

    <template #footer>
      <small v-once>Board {{ boardVersion }} · {{ localeLabel }}</small>
      <slot name="legal" :updated-at="updatedAt">Locally cached.</slot>
    </template>
  </BoardShell>
</template>

<script lang="ts">
export const boardVersion = '2.4.0'
export const boardDensities = ['compact', 'comfortable'] as const
export default { inheritAttrs: false }
</script>

<script setup lang="ts">
import { computed, defineAsyncComponent, onMounted, reactive, ref, watch } from 'vue'
// Components stay explicit so template bindings resemble a production board.
import BoardShell from './BoardShell.vue'
import ConfirmDialog from './ConfirmDialog.vue'
import EmptyState from './EmptyState.vue'
import TaskRow from './TaskRow.vue'

interface Task { id: number; title: string; done: boolean; due: string | null; labels: readonly string[] }
type Filter = 'all' | 'open' | 'done'
type PanelName = 'TaskInspector' | 'ActivityTimeline'

const props = withDefaults(defineProps<{
  boardId: string; endpoint?: string; initialTasks?: Task[]; accent?: string
}>(), {
  endpoint: '/api/tasks',
  initialTasks: () => [],
  accent: '#6d5dfc',
})

const emit = defineEmits<{ saved: [tasks: Task[]]; error: [cause: Error] }>()
const selectedId = defineModel<number | null>('selectedId', { default: null })
const slots = defineSlots<{ footer?: () => unknown; legal?: (props: { updatedAt: Date }) => unknown }>()
const AsyncSummary = defineAsyncComponent(() => import('./AsyncSummary.vue'))

const tasks = reactive<Task[]>(props.initialTasks.length ? props.initialTasks : [
  { id: 101, title: 'Polish café launch copy', done: false, due: '2026-08-01', labels: ['copy'] },
  { id: 102, title: 'Verify 日本語 layout 🚀', done: true, due: null, labels: ['i18n', '𝌆'] },
])
const query = ref(''), filter = ref<Filter>('all'), page = ref(1)
const loading = ref(false), online = ref(true), showDialog = ref(false)
const error = ref<Error | null>(null), lastAction = ref('')
const activePanel = ref<PanelName>('TaskInspector')
const activeAttribute = ref<'title' | 'aria-label'>('title')
const secondaryEvent = 'dblclick' as const
const updatedAt = ref(new Date())

const boardTitle = computed(() => `Mission board ${props.boardId}`)
const activeValue = computed(() => `Search ${tasks.length} tasks`)
const activeSlot = computed(() => slots.legal ? 'append' : 'aside')
const disabled = computed(() => loading.value || !online.value)
const accentColor = computed(() => props.accent)
const localeLabel = 'café · 日本語 · 🚀 · 𝌆'
const introHtml = 'Trusted fixture markup from the operator'
const shellAttributes = { role: 'main', 'data-density': boardDensities[1] }

const visibleTasks = computed(() => tasks.filter((task) => {
  const matchesText = task.title.toLocaleLowerCase().includes(query.value.toLocaleLowerCase())
  const matchesState = filter.value === 'all' || (filter.value === 'done') === task.done
  return matchesText && matchesState
}))
const selectedTask = computed(() => tasks.find(({ id }) => id === selectedId.value) ?? null)
const statusText = computed(() => `${tasks.filter((task) => !task.done).length} tasks remain`)

function isOverdue(task: Task): boolean {
  return Boolean(task.due && !task.done && Date.parse(task.due) < Date.now())
}

function addTask(): void {
  const title = query.value.trim()
  if (!title) return
  tasks.push({ id: Date.now(), title, done: false, due: null, labels: [] })
  query.value = ''
  lastAction.value = `Added “${title}”`
}

function removeTask(id: number): void {
  const index = tasks.findIndex((task) => task.id === id)
  if (index >= 0) tasks.splice(index, 1)
}

function resetFilters(): void { filter.value = 'all'; query.value = '' }

async function refreshBoard(): Promise<void> {
  loading.value = true
  error.value = null
  try {
    const response = await fetch(props.endpoint, { headers: { Accept: 'application/json' } })
    if (!response.ok) throw new Error(`Request failed: ${response.status}`)
    const incoming = await response.json() as Task[]
    tasks.splice(0, tasks.length, ...incoming)
    updatedAt.value = new Date()
    emit('saved', [...tasks])
  } catch (cause: unknown) {
    error.value = cause instanceof Error ? cause : new Error(String(cause))
    emit('error', error.value)
  } finally {
    loading.value = false
  }
}

function confirmRemoval(): void {
  if (selectedId.value != null) removeTask(selectedId.value)
  showDialog.value = false
}

watch(filter, () => { page.value = 1 })
onMounted(() => { online.value = navigator.onLine })
defineExpose({ refresh: refreshBoard, taskCount: computed(() => tasks.length) })
</script>

<style scoped lang="scss">
@use "sass:color";
/* Scoped structure and Vue's style-expression injection share this block. */

$surface: #171923;

.board {
  color: v-bind('accentColor');
  background: color.scale($surface, $lightness: 8%);
  border-radius: 0.75rem;

  &__header {
    display: grid;
    gap: 0.25rem;
    border-block-end: 1px solid color.adjust($surface, $lightness: 20%);
  }
}

.toolbar {
  display: flex;
  flex-wrap: wrap;
  gap: 0.5rem;
  &:focus-within { outline: 2px solid v-bind(accentColor); }
}

:deep(.task-row[data-complete="true"]) { opacity: 0.65; }
:global(body[data-theme="night"]) .notice { color-scheme: dark; }
::v-slotted(.hint) { font-style: italic; }

.task-enter-active,
.task-leave-active { transition: opacity 180ms ease, transform 180ms ease; }
.task-enter-from,
.task-leave-to { opacity: 0; transform: translateY(-0.25rem); }

@media (prefers-reduced-motion: reduce) {
  .task-enter-active, .task-leave-active { transition: none; }
}
</style>

<style module>
/* The shell consumes these names through the generated $style object. */
.panel { min-block-size: 20rem; box-shadow: 0 0 0 1px v-bind(accentColor); }
.active { isolation: isolate; }
@supports (container-type: inline-size) {
  .panel { container-type: inline-size; }
}
</style>
