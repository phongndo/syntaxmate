import Component from '@glimmer/component';
import { tracked } from '@glimmer/tracking';

interface DashboardSignature {
  Args: { title: string; people: Array<{ name: string; active: boolean }> };
}

export default class Dashboard extends Component<DashboardSignature> {
  @tracked selected = 0;
  get heading(): string { return `${this.args.title} — 東京 λ`; }
  <template @title={{this.heading}}>
    {{! NOTE: concise fixture comment 🚀 }}
    <section class="dashboard" data-count={{this.selected}} ...attributes>
      <h1>{{@title}}</h1>
      {{#if @people.length}}
        <People::Card @person={{@people.[0]}} as |card|>
          {{#each @people as |person index|}}
            <span class={{if person.active "ready" "idle"}}>{{index}}: {{person.name}}</span>
          {{else}}
            <em>No people</em>
          {{/each}}
          {{{card.summary}}}
        </People::Card>
      {{else if this.selected}}
        {{yield}}
      {{/if}}
      <input disabled={{false}} {{on "click" this.choose}} /> &amp; 𝌆
    </section>
  </template>
}
