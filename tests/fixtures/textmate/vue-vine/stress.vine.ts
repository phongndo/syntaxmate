import { computed, onMounted, reactive, ref, watch } from 'vue'
import type { Ref } from 'vue'

/** Dashboard fixture: café, Ελληνικά, 日本語, 🚀, and 𝌆. */
interface Metric {
  id: number
  label: string
  value: number
  unit?: 'ms' | '%' | 'req/s'
  trend: 'up' | 'down' | 'flat'
}

type Filter = 'all' | 'healthy' | 'warning'
type MetricMap<T extends Metric = Metric> = Readonly<Record<string, T>>

const seedMetrics: Metric[] = [
  { id: 1, label: 'Latency café', value: 42.5, unit: 'ms', trend: 'down' },
  { id: 2, label: '成功率 🚀', value: 99.95, unit: '%', trend: 'up' },
  { id: 3, label: 'Throughput 𝌆', value: 1200, unit: 'req/s', trend: 'flat' },
]

export function StatusPill(props: { trend: Metric['trend']; compact?: boolean }) {
  const icon = computed(() => ({ up: '↑', down: '↓', flat: '→' })[props.trend])
  vineStyle.scoped(css`
    .status-pill { border-radius: 999px; padding: 0.15rem 0.45rem; }
    .status-pill[data-trend='up'] { color: #087f5b; }
  `);
  return vine`
    <span class="status-pill" :data-trend="trend" :title="'Trend: ' + trend">
      {{ icon }}<slot v-if="!compact" />
    </span>
  `;
}

export function OperationsDashboard(props: { title: string; initial?: Metric[] }) {
  const emit = vineEmits<{
    select: [metric: Metric]
    refresh: [source: 'manual' | 'timer']
  }>()
  const slots = vineSlots<{
    header?: (props: { title: string }) => unknown
    empty?: () => unknown
  }>()
  const selected = ref<number | null>(null)
  const query = ref('')
  const filter = vineModel<Filter>('filter', { default: 'all' })
  const state = reactive({ loading: false, metrics: props.initial ?? seedMetrics })
  const byId: MetricMap = Object.fromEntries(state.metrics.map(item => [item.id, item]))

  const visible = computed(() => state.metrics.filter(metric => {
    const matchesText = metric.label.toLocaleLowerCase().includes(query.value.toLocaleLowerCase())
    const matchesFilter = filter.value === 'all' || metric.value < 90
    return matchesText && matchesFilter
  }))

  async function refresh(source: 'manual' | 'timer' = 'manual'): Promise<void> {
    state.loading = true
    try {
      await Promise.resolve(seedMetrics)
      emit('refresh', source)
    } finally {
      state.loading = false
    }
  }

  function choose(metric: Metric): void {
    selected.value = metric.id
    emit('select', metric)
  }

  watch(query, value => console.debug('query', value))
  onMounted(() => refresh('timer'))
  vineExpose({ refresh, selected })
  vineOptions({ name: 'OperationsDashboard', inheritAttrs: false })

  vineStyle.scoped(css`
    .dashboard {
      --accent: #5f3dc4;
      display: grid;
      gap: 1rem;
      color: #212529;
    }
    .dashboard__title::after { content: ' 🚀'; }
    @media (min-width: 48rem) {
      .dashboard__grid { grid-template-columns: repeat(3, minmax(0, 1fr)); }
    }
  `);

  vineStyle(scss`
    .metric {
      border: 1px solid rgba(0, 0, 0, 0.12);
      &--selected { border-color: var(--accent); }
      &__value { font-variant-numeric: tabular-nums; }
      &:is(:hover, :focus-within) { transform: translateY(-1px); }
    }
  `);

  vineStyle(less`
    @space: 0.75rem;
    .toolbar {
      display: flex;
      gap: @space;
      input { min-inline-size: 12rem; }
    }
  `);

  return vine`
    <main id="ops-dashboard" class="dashboard" :aria-busy="state.loading">
      <!-- Embedded HTML comment: Ελληνικά, 日本語, 🚀, 𝌆. -->
      <header class="dashboard__header">
        <slot name="header" :title="title">
          <h1 class="dashboard__title">{{ title }}</h1>
        </slot>
        <p v-once>Production telemetry &amp; diagnostics</p>
      </header>

      <form class="toolbar" @submit.prevent="refresh('manual')">
        <label for="metric-search">Search</label>
        <input
          id="metric-search"
          v-model.trim="query"
          type="search"
          placeholder="café / 成功率"
          :disabled="state.loading"
        />
        <select v-model="filter" aria-label="Health filter">
          <option value="all">All</option>
          <option value="healthy">Healthy</option>
          <option value="warning">Warning</option>
        </select>
        <button type="submit" :class="{ spinning: state.loading }">Refresh 🚀</button>
      </form>

      <section class="summary" aria-live="polite">
        <span>Selected: {{ selected ?? 'none' }}</span>
        <strong class="summary__count">{{ visible.length }} metrics</strong>
      </section>

      <ul v-if="visible.length" class="dashboard__grid" role="list">
        <li
          v-for="(metric, index) in visible"
          :key="metric.id"
          class="metric"
          :class="{ 'metric--selected': selected === metric.id }"
          :data-index="index"
          @click="choose(metric)"
          @keydown.enter.stop="choose(metric)"
        >
          <h2>{{ metric.label }}</h2>
          <output class="metric__value">{{ metric.value.toFixed(2) }} {{ metric.unit }}</output>
          <StatusPill :trend="metric.trend">{{ metric.trend }}</StatusPill>
          <small v-if="byId[metric.id]">ID #{{ metric.id }}</small>
        </li>
      </ul>
      <div v-else class="empty-state">
        <slot name="empty"><em>No matching telemetry 𝌆</em></slot>
      </div>

      <footer>
        <a href="/reports" target="_blank" rel="noreferrer">Open reports</a>
        <span title='Unicode sample'>café · 日本語 · 🚀</span>
      </footer>
    </main>
  `;
}

export const CompactDashboard = (props: { title?: string }) => vine`
  <OperationsDashboard :title="props.title ?? 'Compact'" filter="healthy" />
`;
