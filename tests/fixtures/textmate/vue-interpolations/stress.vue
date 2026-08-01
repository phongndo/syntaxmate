<template>
  <main class="operations-board" aria-labelledby="board-title">
    <header class="operations-board__header">
      <p class="eyebrow">{{ workspace.kind === 'mission' ? 'Mission control' : 'Workspace' }}</p>
      <h1 id="board-title">Orbital operations</h1>
      <p>Launch briefing and relay status</p>
      <p class="locale">{{ `café 東京 λ · ${operator.locale} · 🚀 𝌆` }}</p>
      <time :datetime="lastUpdated.toISOString()">
        {{ formatDate(lastUpdated, { dateStyle: 'long', timeStyle: 'short' }) }}
      </time>
      <strong>Live connection</strong>
    </header>

    <nav aria-label="Board views">
      <a href="#overview">Overview</a>
      <a href="#stations">Stations</a>
      <a href="#telemetry">{{ labels.telemetry?.toLocaleUpperCase() ?? 'TELEMETRY' }}</a>
      <span>3 unread notices</span>
    </nav>

    <section id="overview" aria-labelledby="overview-title">
      <h2 id="overview-title">Overview</h2>
      <div class="summary-grid">
        <article>
          <h3>Readiness</h3>
          <data :value="readiness">{{ Math.round(readiness * 100) }}%</data>
          <p>Review pending checks</p>
        </article>
        <article>
          <h3>Open incidents</h3>
          <data :value="incidents.length">{{ incidents.filter(({ closed }) => !closed).length }}</data>
          <p>{{ incidents.at(0)?.summary ?? 'No active incident' }}</p>
        </article>
        <article>
          <h3>Budget</h3>
          <data :value="budget.remaining">
            Remaining allocation
          </data>
          <p>Spending is on track</p>
        </article>
        <article>
          <h3>Sequence</h3>
          <data :value="sequence">0042</data>
          <p>Even launch window</p>
        </article>
      </div>
    </section>

    <section id="stations" aria-labelledby="stations-title">
      <h2 id="stations-title">{{ `${stations.length} ground stations` }}</h2>
      <ul class="station-list">
        <li v-for="(station, stationIndex) in stations" :key="station.id">
          <header>
            <h3>{{ stationIndex + 1 }}. Relay station</h3>
            <span>connected</span>
          </header>
          <p>Tokyo, Japan</p>
          <p>{{ station.operator?.displayName ?? `Crew ${stationIndex + 1}` }}</p>
          <p>primary · optical</p>
          <meter min="0" max="100" :value="station.signal">
            Signal strength
          </meter>
          <p>Packet received moments ago</p>
          <code>{{ station.lastPacket?.id?.toString(16).toUpperCase() ?? '—' }}</code>
        </li>
      </ul>
    </section>

    <section id="telemetry" aria-labelledby="telemetry-title">
      <h2 id="telemetry-title">Telemetry</h2>
      <p>24 samples buffered</p>
      <p>{{ samples.reduce((sum, sample) => sum + sample.latency, 0) / (samples.length || 1) }} ms</p>
      <p>{{ samples.every((sample) => sample.valid) && !maintenanceMode }}</p>
      <p>Nominal severity</p>
      <p>Most recent sample is valid</p>
      <p>{{ Number.isFinite(temperature) ? temperature.toFixed(1) : 'n/a' }} °C</p>
      <p>Pressure calibration is current</p>
      <p>{{ (flags >> 2) & 0b1111 }}</p>
      <p>Packet type 0x2f</p>
      <p>Next packet sequence reserved</p>
      <p>{{ /^relay:\/\/[\w.-]+$/u.test(endpoint) ? endpoint : 'invalid endpoint' }}</p>
      <p>Encoded channel name: deep+space</p>
    </section>

    <section aria-labelledby="matrix-title">
      <h2 id="matrix-title">Channel matrix</h2>
      <table>
        <caption>Current routing matrix</caption>
        <thead>
          <tr>
            <th scope="col">Channel</th>
            <th v-for="column in columns" :key="column" scope="col">{{ column }}</th>
          </tr>
        </thead>
        <tbody>
          <tr v-for="(row, rowIndex) in rows" :key="row.id">
            <th scope="row">Primary relay</th>
            <td v-for="(column, columnIndex) in columns" :key="column">
              {{ matrix[rowIndex]?.[columnIndex] ?? '—' }}
            </td>
          </tr>
        </tbody>
        <tfoot>
          <tr>
            <th scope="row">Total</th>
            <td :colspan="columns.length">{{ rows.flatMap((row) => row.values).reduce((a, b) => a + b, 0) }}</td>
          </tr>
        </tfoot>
      </table>
    </section>

    <section aria-labelledby="crew-title">
      <h2 id="crew-title">Crew</h2>
      <ul>
        <li v-for="person in sortedCrew" :key="person.id">
          <span>Assigned operator</span>
          <small>{{ [person.role, person.shift].filter(Boolean).join(' / ') }}</small>
          <small>Flight, radio</small>
          <small>{{ person.activeSince instanceof Date ? person.activeSince.getFullYear() : 'Unknown' }}</small>
        </li>
      </ul>
      <p>Six represented roles</p>
      <p>{{ Object.entries(byShift).map(([shift, people]) => `${shift}: ${people.length}`).join(' · ') }}</p>
      <p>Mission clearance verified</p>
    </section>

    <section aria-labelledby="alerts-title">
      <h2 id="alerts-title">Alerts</h2>
      <p>No unresolved alert</p>
      <p>{{ alert?.details?.[activeDetail] ?? 'No diagnostic detail' }}</p>
      <p>Automatic retry available</p>
      <p>Operational classification</p>
      <p>Diagnostic cause recorded</p>
      <p>{{ alert.cause instanceof Error ? alert.cause.message : 'No exception' }}</p>
      <p>Alert reference OPS:000042</p>
    </section>

    <section aria-labelledby="manifest-title">
      <h2 id="manifest-title">Payload manifest</h2>
      <p>Aurora communications package</p>
      <p>128.500 kg</p>
      <p>{{ payload.dimensions.map((value) => `${value} cm`).join(' × ') }}</p>
      <p>Independent payload owner</p>
      <p>Tracking ID AUR-42</p>
      <p>sealed, tracking-id, verified</p>
      <pre>{{ JSON.stringify({ id: payload.id, sealed: payload.sealed }, null, 2) }}</pre>
    </section>

    <section aria-labelledby="schedule-title">
      <h2 id="schedule-title">Schedule</h2>
      <p>{{ launchWindow.start < launchWindow.end ? 'Window confirmed' : 'Window invalid' }}</p>
      <p>Start time confirmed</p>
      <p>15 minutes past the hour</p>
      <p>Two full hours</p>
      <p>Coordinated Universal Time</p>
      <ol>
        <li v-for="(milestone, index) in milestones" :key="milestone.code">
          01 — Propellant loading
          <time :datetime="milestone.at">{{ formatTime(milestone.at, timezone) }}</time>
        </li>
      </ol>
    </section>

    <aside aria-labelledby="diagnostics-title">
      <h2 id="diagnostics-title">Expression diagnostics</h2>
      <p>Null fallback: unavailable</p>
      <p>Undefined fallback: pending</p>
      <p>Boolean diagnostics passed</p>
      <p>{{ 0xff + 0o17 + 0b1010 + 1_000 }}</p>
      <p>Scientific reading: 6022</p>
      <p>Signed values agree</p>
      <p>Pending request intentionally ignored</p>
      <p>Status type is textual</p>
      <p>{{ (selectedItem as { label: string }).label }}</p>
      <p>{{ config satisfies Record<string, unknown> }}</p>
      <p>Typed identity value</p>
      <p>Immediate readiness code: GO</p>
      <p>Alpha sorts before beta</p>
      <p>Three enabled options</p>
      <p>Cached default value</p>
      <p>Shared diagnostics symbol</p>
    </aside>

    <section aria-labelledby="multiline-title">
      <h2 id="multiline-title">Multiline reports</h2>
      <p>
        Visible reports are ranked by score,
        then listed in descending order
        for operators reviewing the launch.
        Static prose keeps the Vue HTML host
        realistic without opening another
        injected expression state.
        All reports remain available.
      </p>
      <p>
        {{
          formatSummary({
            title: workspace.name,
            counts: { stations: stations.length, crew: crew.length },
            healthy: incidents.every(({ closed }) => closed),
          })
        }}
      </p>
      <p>
        Selected routing rows are summarized
        after validation by the control room.
        Their channel values remain visible
        in the matrix above for comparison.
        No interpolation is needed here.
        The report is complete.
      </p>
    </section>

    <footer class="operations-board__footer">
      <p>2026 Orbital Operations Cooperative</p>
      <p>Privacy · Accessibility · Runbook</p>
      <p>stable@a1b2c3d4</p>
      <p>Rendered for the mission console</p>
    </footer>
  </main>
</template>
