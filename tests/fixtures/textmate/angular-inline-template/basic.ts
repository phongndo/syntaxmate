@Component({
  selector: 'app-greeting',
  template: `
    <article class="card" lang="fr">
      <h1>{{ title ?? 'café 🚀 𝌆' }}</h1>
      <p data-city="東京">Hello, {{ user?.name }}!</p>
      @if (ready) { <strong>ready</strong> }
    </article>
  `,
  styles: ['.card { display: grid; }'],
})
export class GreetingComponent {}

const nested = {
  template: ((`<button title="東京">{{ label }}</button>`)),
  changeDetection: 0,
}

template: '<span>{{ value | uppercase }}</span>', other: true
