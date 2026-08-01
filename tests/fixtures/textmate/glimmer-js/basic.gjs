import Component from '@glimmer/component';
import { on } from '@ember/modifier';

const launchLabel = '東京 λ 🚀 𝌆';

export default class LaunchCard extends Component {
  get heading() {
    return `${launchLabel}: ${this.args.title}`;
  }

  <template>
    {{!-- NOTE: a compact component exercising core Glimmer forms --}}
    <article class="launch-card {{if @featured 'featured' 'standard'}}" data-city="東京">
      <h2>{{this.heading}}</h2>
      {{#if @mission}}
        <Mission::Badge @mission={{@mission}} @active={{true}} />
      {{else}}
        <p title='λ and 🚀'>No mission &amp; no telemetry.</p>
      {{/if}}
      <ul>
        {{#each @crew as |member index|}}<li>{{index}}: {{member.name}}</li>{{/each}}
      </ul>
      <button type="button" {{on "click" @launch}}>Launch {{@rocket}}</button>
      {{{@trustedStatus}}}
    </article>
  </template>
}
