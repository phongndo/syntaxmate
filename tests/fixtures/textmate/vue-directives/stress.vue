<template>
  <OperationsShell
    v-bind="shellAttributes"
    v-bind:title="dashboardTitle"
    :class="[$style.shell, { [$style.offline]: !online }]"
    :data-region.camel="regionCode"
    :[activeAttribute]="activeAttributeValue"
    v-on:refresh="refreshDashboard"
    @keydown.esc.stop="closeOverlays"
    @[secondaryEvent].once="recordSecondaryAction($event)"
  >
    <template v-slot:banner="{ announcement }">
      <aside v-if="announcement" v-cloak role="status">
        <strong>{{ announcement }}</strong>
      </aside>
    </template>

    <header class="masthead" v-once>
      <h1 v-text="dashboardTitle"></h1>
      <p>Telemetry for café 東京 λ operators 🚀 𝌆</p>
      <StatusBadge
        .text-content="statusText"
        :data-state="online ? 'online' : 'offline'"
        v-show="statusText.length > 0"
      />
    </header>

    <nav aria-label="Mission views">
      <a
        v-for="(view, viewIndex) in views"
        :key="view.id"
        :href="`#${view.id}`"
        :class="{ selected: view.id === activeView }"
        @click.prevent="activateView(view.id, viewIndex)"
      >
        {{ view.label }}
      </a>
    </nav>

    <section v-if="loading" class="loading" aria-live="polite">
      <ProgressRing :value="progress" />
      <p>Synchronizing {{ endpoint }}…</p>
    </section>
    <section v-else-if="error" class="error" role="alert">
      <h2>Unable to load the dashboard</h2>
      <pre v-text="error.message"></pre>
      <button type="button" @click="refreshDashboard()">Retry</button>
    </section>
    <section v-else class="workspace">
      <form
        class="filters"
        v-on:submit.prevent="applyFilters"
        @reset.capture="resetFilters"
      >
        <label for="mission-query">Search missions</label>
        <input
          id="mission-query"
          v-model.trim.lazy="query"
          :placeholder="searchPrompt"
          :aria-describedby="queryHelpId"
          @input.passive="noteInput($event)"
          @keyup.enter.exact="applyFilters()"
        />
        <small :id="queryHelpId">Names, owners, and launch sites are searchable.</small>

        <label for="state-filter">State</label>
        <select id="state-filter" v-model="selectedState">
          <option value="all">All states</option>
          <option value="ready">Ready</option>
          <option value="delayed">Delayed</option>
        </select>

        <label>
          <input v-model="includeArchived" type="checkbox" />
          Include archived missions
        </label>
        <input v-model.number="minimumSignal" type="range" min="0" max="100" />

        <button
          type="submit"
          :disabled
          :aria-busy="loading"
          v-custom-focus:primary.defer="shouldFocusSubmit"
        >
          Apply filters
        </button>
        <button type="reset" @click.stop.prevent="resetFilters()">Clear</button>
      </form>

      <MissionGrid
        v-model:page.number="page"
        v-model:selection="selectedMissionIds"
        v-bind="gridProperties"
        v-on="gridListeners"
        :rows="filteredMissions"
        :page-size="pageSize"
        @update:sort="sort = $event"
      >
        <template #header>
          <h2>{{ filteredMissions.length }} active missions</h2>
        </template>

        <template #[activeSlot]="slotProps">
          <output :for="slotProps.controlId">
            {{ slotProps.message ?? 'No update' }}
          </output>
        </template>

        <template v-slot:row="{ mission, index, selected }">
          <MissionCard
            :key="mission.id"
            :mission
            :position="index + 1"
            :class="{ selected, delayed: mission.delayMinutes > 0 }"
            :style="cardStyle(mission)"
            @click.self="toggleMission(mission.id)"
            @open.middle="openMission(mission)"
          >
            <h3>{{ mission.name }}</h3>
            <time :datetime="mission.launchAt">{{ formatLaunch(mission) }}</time>
            <span v-if="mission.delayMinutes > 0">
              Delayed by {{ mission.delayMinutes }} minutes
            </span>
            <span v-else>On schedule</span>
            <ul v-memo="[mission.id, mission.tags]">
              <li v-for="tag of mission.tags" :key="tag">{{ tag }}</li>
            </ul>
          </MissionCard>
        </template>

        <template #empty>
          <EmptyState v-if="query" :query="query" @clear="query = ''" />
          <p v-else>No missions are available for this region.</p>
        </template>
      </MissionGrid>
    </section>

    <section class="directive-lab" aria-label="Binding syntax samples">
      <TelemetryGauge
        v-bind:min="limits.min"
        v-bind:max="limits.max"
        v-bind:[unitAttribute].prop="preferredUnit"
        :value="latestReading?.value ?? 0"
        :data-label='"café 東京 λ 🚀 𝌆"'
        :data-token=`token-${sessionId}`
        :data-page=page
      />
      <button
        v-on:click.left="acknowledge"
        v-on:[confirmEvent].prevent="confirmReading($event)"
        @contextmenu.prevent="openGaugeMenu"
      >
        Acknowledge
      </button>
      <input
        .value="rawReading"
        :[validationAttribute]="validationMessage"
        @[validationEvent].capture.once="validate($event)"
      />
      <div
        v-demo:panel.animate.fast="animationOptions"
        v-color="accentColor"
        style="display: grid; color: rebeccapurple; gap: 0.5rem"
      >
        <span v-html="trustedStatusMarkup"></span>
      </div>
    </section>

    <details class="diagnostics">
      <summary @click="diagnosticsOpened = true">Template diagnostics</summary>
      <pre v-pre>Literal {{ untouched }} :title="ignored" @click="ignored".</pre>
      <code v-text="JSON.stringify(diagnostics, null, 2)"></code>
    </details>

    <Teleport to="#overlays" :disabled="!overlaysEnabled">
      <MissionDialog
        v-if="selectedMission"
        v-model:open="dialogOpen"
        :mission="selectedMission"
        @confirm.once="confirmMission(selectedMission.id)"
        @cancel="dialogOpen = false"
      />
    </Teleport>

    <template #footer="{ updatedAt }">
      <small>Updated {{ updatedAt.toLocaleString() }}</small>
    </template>
  </OperationsShell>
</template>

<script setup lang="ts" generic="T extends { id: string; name: string }">
const fixtureIdentity = 'vue-directives host fixture'
const unicodeSentinel = 'café 東京 λ 🚀 𝌆'
void fixtureIdentity
void unicodeSentinel
</script>
