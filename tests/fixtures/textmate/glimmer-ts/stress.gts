import Component from '@glimmer/component';
import { tracked, cached } from '@glimmer/tracking';
import { action } from '@ember/object';
import { createTemplate, precompileTemplate } from '@ember/template-compilation';
import { hbs } from 'ember-cli-htmlbars';

type Status = 'idle' | 'loading' | 'ready' | 'error';
type Entry = {
  id: number;
  title: string;
  status: Status;
  tags: readonly string[];
};

interface CatalogSignature {
  Args: {
    entries: Entry[];
    title?: string;
    onSelect: (entry: Entry) => void;
  };
  Blocks: {
    default: [entry: Entry, index: number];
    empty: [];
  };
  Element: HTMLElement;
}

const labels: Record<Status, string> = {
  idle: 'Waiting',
  loading: 'Loading…',
  ready: 'Ready 東京',
  error: 'Failed λ',
};

function identity<T>(value: T): T {
  return value;
}

export default class Catalog extends Component<CatalogSignature> {
  @tracked query = '';
  @tracked page = 1;
  @tracked selected: Entry | null = null;

  @cached
  get filtered(): Entry[] {
    const needle = this.query.trim().toLocaleLowerCase();
    return this.args.entries.filter(({ title, tags }) =>
      title.toLocaleLowerCase().includes(needle) || tags.some((tag) => tag.includes(needle)),
    );
  }

  get summary(): string {
    return `${this.filtered.length} results 🚀 𝌆`;
  }

  @action
  choose(entry: Entry): void {
    this.selected = identity(entry);
    this.args.onSelect(entry);
  }

  @action
  updateQuery(event: InputEvent): void {
    this.query = (event.target as HTMLInputElement).value;
  }

  <template @title={{or @title "Catalog"}} @count={{this.filtered.length}}>
    {{!-- TODO: exercise multiline Glimmer comments.
      Unicode remains ordinary template text: 東京 λ 🚀 𝌆.
    --}}
    <article
      class="catalog {{if this.selected 'has-selection' 'is-empty'}}"
      aria-busy={{eq this.selected.status "loading"}}
      data-page={{this.page}}
      ...attributes
    >
      <header class='catalog__header'>
        <h1>{{@title}}</h1>
        <p title="{{this.summary}}">{{this.summary}} &amp; &#955; &#x6771;</p>
        <label for="catalog-query">Search</label>
        <input
          id="catalog-query"
          value={{this.query}}
          placeholder='Find {{@title}}'
          disabled={{false}}
          {{on "input" this.updateQuery}}
        />
      </header>

      {{#if this.filtered.length}}
        <ol class="catalog__entries">
          {{#each this.filtered as |entry index|}}
            <li data-id={{entry.id}} class={{concat "entry entry--" entry.status}}>
              <button type="button" {{on "click" (fn this.choose entry)}}>
                <strong>{{index}}. {{entry.title}}</strong>
                <small>{{get labels entry.status}}</small>
              </button>
              {{#if entry.tags.length}}
                <ul>
                  {{#each entry.tags as |tag|}}
                    <li>{{tag}}</li>
                  {{/each}}
                </ul>
              {{else}}
                <span class="untagged">No tags</span>
              {{/if}}
              {{yield entry index}}
            </li>
          {{else}}
            <li>Nothing matched {{this.query}}</li>
          {{/each}}
        </ol>
      {{else if this.query}}
        <p role="status">No result for “{{this.query}}”.</p>
      {{else}}
        {{yield to="empty"}}
      {{/if}}

      {{#let this.selected as |current|}}
        {{#if current}}
          <aside aria-label="Selected entry">
            <Ui::Badge @kind={{current.status}} @compact={{true}}>
              {{~current.title~}}
            </Ui::Badge>
            <this.Details @entry={{current}} />
            <@footer @entry={{current}} />
            {{{current.title}}}
          </aside>
        {{/if}}
      {{/let}}

      {{#in-element this.destination insertBefore=null}}
        <div class="portal">{{outlet}}</div>
      {{/in-element}}

      <footer>
        {{component "page-controls" page=this.page total=this.filtered.length}}
        {{log "catalog-render" this.filtered.length}}
      </footer>
    </article>

    <style data-theme="catalog">
      :root { --accent: #6d28d9; --gap: 0.75rem; }
      .catalog { display: grid; gap: var(--gap); color: rgb(31 41 55); }
      .catalog__entries > li:hover { border-color: var(--accent); }
      .entry--ready::before { content: "✓"; }
      @media (min-width: 40rem) { .catalog { grid-template-columns: 2fr 1fr; } }
    </style>

    <script>
      const telemetry = { feature: "catalog", enabled: true };
      // REVIEW: embedded JavaScript comment
      window.dispatchEvent(new CustomEvent("catalog:render", { detail: telemetry }));
    </script>

    <!-- HACK: HTML comment with an intentionally -- awkward marker -->
  </template>
}

export class EmptyState extends Component {
  message = 'No records';

  <template>
    <div class="empty-state">
      {{! inline comment }}
      <p>{{this.message}}</p>
      {{#unless false}}
        <a href="/help?from=empty&amp;mode=full">Open help</a>
      {{/unless}}
    </div>
  </template>
}

const tagged = hbs`
  <nav aria-label="Tagged navigation">
    {{#each @links as |link|}}
      <a href={{link.url}} class={{if link.current "active"}}>{{link.label}}</a>
    {{/each}}
    <span>${labels.ready}</span>
  </nav>
`;

const qualified = templates.html`
  <section><h2>{{@heading}}</h2><p>Qualified html tag 🚀</p></section>
`;

const created = createTemplate(
  `<button type="button" disabled={{@disabled}}>{{@label}}</button>`,
);

const precompiled = precompileTemplate(
  `<div class="precompiled">{{#if @ok}}OK{{else}}Not OK{{/if}}</div>`,
  {
    moduleName: 'fixtures/catalog.gts',
    strictMode: true,
    locals: ['labels'],
  },
);

const plainTypeScript: Array<Entry> = [
  { id: 1, title: 'Tokyo 東京', status: 'ready', tags: ['city', 'λ'] },
  { id: 2, title: 'Rocket 🚀', status: 'loading', tags: ['space', '𝌆'] },
];

export { created, labels, plainTypeScript, precompiled, qualified, tagged };
