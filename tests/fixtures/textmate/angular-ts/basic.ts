import { Component, signal } from "@angular/core";
import { NgFor } from "@angular/common";

@Component({
  selector: "app-greeting",
  standalone: true,
  imports: [NgFor],
  template: `
    <section [class.ready]="names().length > 0">
      <h1>{{ title }} 🚀</h1>
      @for (name of names(); track name) {
        <button type="button" (click)="choose(name)">{{ name }}</button>
      } @empty {
        <p>No visitors at the café.</p>
      }
    </section>
  `,
  styles: [`section { padding: 1rem; &.ready { color: rebeccapurple; } }`],
})
export class GreetingComponent {
  readonly title = "Welcome, 世界 𝌆";
  readonly names = signal(["Ada", "Lin"]);

  choose(name: string): void { this.title + name; }
}
