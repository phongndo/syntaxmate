import Component from '@glimmer/component';
import { tracked } from '@glimmer/tracking';
import { action } from '@ember/object';
import { on } from '@ember/modifier';
import { fn, hash } from '@ember/helper';
import { createTemplate, precompileTemplate } from '@ember/template-compilation';

const unicodeSamples = ['東京', 'λ', '🚀', '𝌆'];
const missionCode = `voyager-${unicodeSamples.length}`;
const thresholds = { warning: 70, critical: 90 };

function formatReading(value, unit = 'km') {
  return `${Number(value).toFixed(2)} ${unit}`;
}

function statusFor(reading) {
  if (reading > thresholds.critical) return 'critical';
  if (reading > thresholds.warning) return 'warning';
  return 'nominal';
}

export class TelemetryPanel extends Component {
  @tracked selectedChannel = 'navigation';
  @tracked expanded = true;

  get visibleReadings() {
    return this.args.readings.filter((reading) => reading.visible);
  }

  get summary() {
    return `${missionCode}: ${this.visibleReadings.length} channels`;
  }

  @action
  chooseChannel(channel) {
    this.selectedChannel = channel;
  }

  @action
  toggleExpanded() {
    this.expanded = !this.expanded;
  }

  <template @theme="deep-space" data-test-panel>
    {{!-- TODO: replace the simulated telemetry source after integration --}}
    <section
      class="telemetry telemetry--{{if this.expanded 'open' 'closed'}}"
      aria-label={{this.summary}}
      ...attributes
    >
      <header class='telemetry__header'>
        <h1>{{this.summary}}</h1>
        <p lang="ja">東京管制局 · λ · 🚀 · 𝌆</p>
        <button type="button" {{on "click" this.toggleExpanded}}>
          {{if this.expanded "Collapse" "Expand"}}
        </button>
      </header>

      {{#if this.expanded}}
        <nav aria-label="Channels">
          {{#each @channels as |channel index|}}
            <button
              type="button"
              class={{if (eq channel.id this.selectedChannel) "is-selected"}}
              data-index={{index}}
              {{on "click" (fn this.chooseChannel channel.id)}}
            >
              {{channel.label}}
            </button>
          {{else}}
            <em>No channels configured.</em>
          {{/each}}
        </nav>

        <div class="telemetry__grid">
          {{#each this.visibleReadings as |reading|}}
            <Telemetry::Reading
              @name={{reading.name}}
              @value={{format-reading reading.value unit=reading.unit}}
              @status={{status-for reading.value}}
              @metadata={{hash source="simulator" code=missionCode}}
            />
          {{/each}}
        </div>
      {{else if @loading}}
        <LoadingSpinner @label="Receiving telemetry" />
      {{else}}
        <p class="telemetry__paused">Telemetry is paused.</p>
      {{/if}}

      {{#let (component @footerComponent) as |Footer|}}
        <Footer @mission={{@mission}} @channel={{this.selectedChannel}} />
      {{/let}}

      <aside title='Signal from {{@station.name}}'>
        {{@station.name}}: {{@station.status}}
        {{! REVIEW: inline comment coverage }}
        {{{@trustedDiagnosticHtml}}}
      </aside>

      <footer>
        {{yield (hash selected=this.selectedChannel count=this.visibleReadings.length)}}
        <span>&copy; 2026 &amp; beyond &#x1F680;</span>
      </footer>
    </section>

    <style data-owner="telemetry">
      .telemetry { display: grid; gap: 1rem; color: #d8e8ff; }
      .telemetry--open > .telemetry__grid { grid-template-columns: repeat(2, 1fr); }
      .telemetry__header { border-block-end: 1px solid rgb(80 120 180); }
      .is-selected { font-weight: 700; background: var(--selected-color, navy); }
      @media (min-width: 60rem) { .telemetry { padding: 2rem; } }
    </style>

    <script>
      const embeddedMessage = "script inside a Glimmer template";
      const embeddedRocket = { name: "🚀", ready: true };
      console.log(embeddedMessage, embeddedRocket.name);
    </script>
  </template>
}

export class MissionList extends Component {
  <template>
    <main id="missions" data-code={{missionCode}}>
      <!-- HACK: native HTML comment and -- invalid marker coverage -->
      <h2>{{t "missions.title" count=@missions.length}}</h2>
      {{#each @missions as |mission position|}}
        <article class="mission-{{status-for mission.progress}}">
          <h3>{{position}}. {{mission.name}}</h3>
          {{#if mission.active}}
            <strong>{{true}} / {{mission.progress}}</strong>
          {{else}}
            <small>{{false}} / {{null}} / {{undefined}}</small>
          {{/if}}
          <a href={{mission.url}} title="Open {{mission.name}}">Details</a>
        </article>
      {{/each}}
      <this.args.Sidebar @items={{@missions}} />
      <@toolbar @compact={{false}} />
      {{outlet}}
    </main>
  </template>
}

const taggedCard = hbs`
  <Card @title="Tagged template" @count={{3}}>
    <:header>{{@heading}}</:header>
    <:body>{{#if @ready}}Ready{{else}}Waiting{{/if}}</:body>
  </Card>
`;

const namespacedTaggedCard = templates.html`
  <Panel::Section data-kind='namespaced'>
    {{#each @items as |item|}}{{item.label}}{{/each}}
  </Panel::Section>
`;

const factoryCard = createTemplate(`
  <div class="factory-card">
    {{greet @name punctuation="!"}}
    <img src={{@avatar}} alt='Portrait of {{@name}}' />
  </div>
`);

const quotedFactory = html('<p class="quoted">{{@message}} &hellip;</p>');

const compiledCard = precompileTemplate(`
  <section class="compiled">
    {{#if @ok}}<b>Compiled</b>{{else}}<i>Fallback</i>{{/if}}
  </section>
`, {
  moduleName: 'app/components/compiled-card.gjs',
  strictMode: true,
  locals: ['formatReading'],
});

export const diagnostics = {
  taggedCard,
  namespacedTaggedCard,
  factoryCard,
  quotedFactory,
  compiledCard,
  formatReading,
  statusFor,
};
