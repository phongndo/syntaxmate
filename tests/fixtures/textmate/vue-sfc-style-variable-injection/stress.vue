<template>
  <main class="flight-board" :data-mode="mode">
    <header class="flight-board__header">
      <div>
        <p class="eyebrow">Deep-space operations</p>
        <h1>{{ title }}</h1>
        <p class="locales">café · 東京 · λ lab · 🚀 · 𝌆 archive</p>
      </div>
      <button class="mode-toggle" type="button" @click="toggleMode">
        Use {{ mode === 'night' ? 'day' : 'night' }} mode
      </button>
    </header>

    <section class="toolbar" aria-label="Flight filters">
      <label>
        Search
        <input v-model.trim="query" type="search" placeholder="Mission or crew" />
      </label>
      <label>
        Density
        <select v-model="density">
          <option value="compact">Compact</option>
          <option value="comfortable">Comfortable</option>
        </select>
      </label>
      <output>{{ filteredFlights.length }} flights</output>
    </section>

    <ol class="flight-grid">
      <li
        v-for="flight in filteredFlights"
        :key="flight.id"
        class="flight-card"
        :class="{ 'flight-card--alert': flight.alert }"
      >
        <span class="flight-card__code">{{ flight.code }}</span>
        <strong>{{ flight.destination }}</strong>
        <span class="flight-card__crew">{{ flight.crew }} crew</span>
        <span class="flight-card__status">{{ flight.status }}</span>
      </li>
    </ol>

    <aside class="status-strip" aria-live="polite">
      <span class="status-strip__signal">Signal {{ signal }}%</span>
      <span class="status-strip__label">{{ statusMessage }}</span>
    </aside>
  </main>
</template>

<script setup lang="ts">
import { computed, reactive, ref } from 'vue'

type Mode = 'day' | 'night'
type Density = 'compact' | 'comfortable'
type Flight = {
  id: number
  code: string
  destination: string
  crew: number
  status: 'boarding' | 'ready' | 'delayed'
  alert: boolean
}

const title = 'Orbital flight board'
const mode = ref<Mode>('night')
const density = ref<Density>('comfortable')
const query = ref('')
const signal = ref(97)
const viewport = reactive({ width: 1440, height: 900 })
const metrics = reactive({ radius: 14, tight: 10, roomy: 18, gap: 16 })
const font = reactive({ primary: 'Inter', fallback: 'system-ui' })
const theme = reactive({
  palette: {
    day: { surface: '#f8fafc', text: '#172033', border: '#94a3b8' },
    night: { surface: '#111827', text: '#e5f2ff', border: '#475569' },
    accent: '#38bdf8',
    warning: '#fb7185',
    fallback: { border: '#64748b' },
  },
  shadow: { color: 'rgb(15 23 42 / 0.4)', strength: 0.42 },
  motion: { quick: '140ms', calm: '320ms' },
})

const flights = ref<Flight[]>([
  { id: 1, code: 'AX-17', destination: 'Luna Gateway', crew: 6, status: 'ready', alert: false },
  { id: 2, code: 'TK-42', destination: '東京 Relay', crew: 4, status: 'boarding', alert: false },
  { id: 3, code: 'LM-09', destination: 'λ Research Ring', crew: 3, status: 'delayed', alert: true },
])

const filteredFlights = computed(() => {
  const needle = query.value.toLocaleLowerCase()
  return flights.value.filter((flight) =>
    `${flight.code} ${flight.destination}`.toLocaleLowerCase().includes(needle),
  )
})

const statusMessage = computed(() =>
  signal.value > 90 ? 'Telemetry nominal' : 'Telemetry degraded',
)

function toggleMode() {
  mode.value = mode.value === 'night' ? 'day' : 'night'
}

function resolveSpace(kind: Density, width: number) {
  const base = kind === 'compact' ? metrics.tight : metrics.roomy
  return `${width < 900 ? base * 0.75 : base}px`
}

function panelShadow(color: string, strength: number) {
  return `0 1rem 3rem color-mix(in srgb, ${color} ${strength * 100}%, transparent)`
}
</script>

<style scoped>
.flight-board {
  /* v-bind(theme.palette.commentOnly) is excluded inside this block comment. */
  min-block-size: 100vh;
  padding: v-bind(resolveSpace(density, viewport.width));
  color: v-bind(theme.palette[mode]?.text ?? '#ffffff');
  background-color: v-bind("theme.palette[mode].surface");
  font-family: v-bind([font.primary, font.fallback].join(', '));
}

.flight-board__header {
  display: flex;
  align-items: center;
  justify-content: space-between;
  gap: v-bind(`${metrics.gap}px`);
  border-block-end: 1px solid v-bind('theme.palette.accent');
}

.eyebrow {
  color: v-bind(theme.palette.accent);
  letter-spacing: 0.12em;
  text-transform: uppercase;
}

.locales {
  opacity: v-bind(signal > 90 ? 0.82 : 0.58);
}

.mode-toggle {
  border: 1px solid v-bind(theme.palette[mode]?.border ?? theme.palette.fallback.border);
  border-radius: v-bind(Math.max(metrics.radius, 4) + 'px');
  transition: color v-bind(theme.motion.quick), background v-bind("theme.motion.calm");
}

.flight-grid {
  display: grid;
  grid-template-columns: repeat(auto-fit, minmax(14rem, 1fr));
  gap: v-bind((density === 'compact' ? metrics.tight : metrics.roomy) + 'px');
  padding: 0;
  list-style: none;
}

.flight-card {
  display: grid;
  gap: v-bind   (resolveSpace(density, viewport.width));
  padding: v-bind(
    resolveSpace(density === 'compact' ? 'compact' : 'comfortable', viewport.width)
  );
  border: 1px solid v-bind(theme.palette[mode].border);
  border-radius: v-bind(`${metrics.radius}px`);
  box-shadow: v-bind(panelShadow(theme.shadow.color, theme.shadow.strength));
}

.flight-card--alert {
  border-color: v-bind('theme.palette.warning');
}
</style>

<style lang="postcss" scoped>
.toolbar {
  /* v-bind(theme.motion.commentOnly) is inert in a PostCSS comment. */
  display: flex;
  flex-wrap: wrap;
  gap: v-bind(resolveSpace(density, viewport.width));
  margin-block: v-bind(`${metrics.gap * 1.5}px`);

  & label {
    color: v-bind("theme.palette[mode].text");
  }

  & input,
  & select {
    outline-color: v-bind(theme.palette.accent ?? '#0ea5e9');
    transition-duration: v-bind('theme.motion.quick');
  }

  @media (width < 48rem) {
    align-items: stretch;
    flex-direction: column;
  }
}
</style>

<style lang="sass" scoped>
.flight-card__code
  // v-bind(theme.palette.commentOnly) stays a Sass comment.
  color: v-bind(theme.palette.accent)
  font-weight: 700
  letter-spacing: v-bind((metrics.gap / 100) + 'em')

.flight-card__crew
  opacity: v-bind(signal >= 95 ? 0.8 : 1)

.flight-card__status
  border-inline-start: v-bind(`${Math.max(metrics.radius / 4, 2)}px`) solid v-bind('theme.palette.accent')
  padding-inline-start: v-bind(resolveSpace('compact', viewport.width))
</style>

<style lang="stylus" scoped>
.status-strip
  // v-bind(theme.shadow.commentOnly) stays a Stylus comment.
  display flex
  justify-content space-between
  margin-block-start v-bind(`${metrics.gap * 2}px`)
  padding v-bind(resolveSpace(density, viewport.width))
  color v-bind("theme.palette[mode].text")
  background v-bind(theme.palette[mode]?.surface ?? '#111827')
  box-shadow v-bind(panelShadow(theme.shadow.color, theme.shadow.strength))

  &__signal
    color v-bind(signal > 90 ? theme.palette.accent : theme.palette.warning)

  &__label
    transition-duration v-bind('theme.motion.calm')
</style>
