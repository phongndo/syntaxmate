import { CommonModule } from "@angular/common";
import { FormsModule } from "@angular/forms";
import {
  Component, Directive, ElementRef, EventEmitter, HostBinding, HostListener,
  Inject, Injectable, InjectionToken, Input, Output, Pipe, PipeTransform,
  ViewChild, computed, inject, signal,
} from "@angular/core";

type MissionState = "queued" | "active" | "complete";
type MissionId = `mission-${number}`;

interface Mission {
  readonly id: MissionId;
  title: string;
  owner?: string;
  state: MissionState;
  progress: number;
  tags: readonly string[];
}

const DEFAULT_LIMIT = new InjectionToken<number>("mission.limit", {
  providedIn: "root",
  factory: () => 12,
});

@Injectable({ providedIn: "root" })
export class MissionStore {
  private readonly sequence = signal(3);
  readonly missions = signal<Mission[]>([
    { id: "mission-1", title: "Map the café", owner: "Renée", state: "active", progress: 62, tags: ["maps", "日本語"] },
    { id: "mission-2", title: "Launch telescope 🚀", state: "queued", progress: 0, tags: ["space", "𝌆"] },
  ]);
  readonly completed = computed(() => this.missions().filter(({ state }) => state === "complete").length);

  constructor(@Inject(DEFAULT_LIMIT) readonly limit: number) {}

  add(title: string): Mission {
    this.sequence.update(value => value + 1);
    const id = `mission-${this.sequence()}` as MissionId;
    const mission: Mission = { id, title, state: "queued", progress: 0, tags: [] };
    this.missions.update(items => [...items, mission].slice(-this.limit));
    return mission;
  }

  remove(id: MissionId): void {
    this.missions.update(items => items.filter(item => item.id !== id));
  }

}

@Pipe({ name: "missionLabel", standalone: true })
export class MissionLabelPipe implements PipeTransform {
  transform(value: Mission, prefix = "Mission"): string {
    return `${prefix}: ${value.title} (${Math.round(value.progress)}%)`;
  }
}

@Directive({ selector: "[appFocusOn]", standalone: true })
export class FocusOnDirective {
  @Input({ required: true }) appFocusOn = false;
  @Output() focused = new EventEmitter<HTMLElement>();
  @HostBinding("class.focus-requested") get requested(): boolean { return this.appFocusOn; }

  constructor(private readonly element: ElementRef<HTMLElement>) {}

  @HostListener("mouseenter")
  focusWhenRequested(): void {
    if (this.appFocusOn) {
      this.element.nativeElement.focus();
      this.focused.emit(this.element.nativeElement);
    }
  }
}

@Component({
  selector: "app-mission-board",
  standalone: true,
  imports: [CommonModule, FormsModule, FocusOnDirective, MissionLabelPipe],
  host: {
    "[class.compact]": "compact",
    "[attr.data-count]": "visible().length",
    "(keydown.escape)": "clearSelection()",
  },
  template: `
    <main class="board" [attr.aria-busy]="loading()" [style.--accent]="accent">
      <!-- Angular template fixture: café, Ελληνικά, 日本語, and satellite 🛰️. -->
      <header>
        <p class="eyebrow">{{ heading | uppercase }}</p>
        <h1 #titleHeading>{{ heading }}</h1>
        <p>{{ store.completed() }} complete / {{ store.missions().length }} total</p>
      </header>

      <form (ngSubmit)="create()" class="toolbar">
        <label for="mission-query">Filter missions</label>
        <input id="mission-query" name="query" [(ngModel)]="query"
          [appFocusOn]="visible().length === 0" (focused)="noteFocus($event)"
          placeholder="Search café missions" />
        <button type="submit" [disabled]="!query.trim()">Add</button>
        <button type="button" (click)="compact = !compact">Toggle density</button>
      </form>

      @let selectedMission = selected();
      @if (error(); as message) {
        <p role="alert" class="error">{{ message }}</p>
      } @else if (loading()) {
        <p class="loading">Loading telemetry…</p>
      } @else {
        <p [class.muted]="!selectedMission">
          {{ selectedMission?.title ?? "Choose a mission" }}
        </p>
      }

      <ul class="missions">
        @for (mission of visible(); track mission.id; let index = $index, first = $first) {
          <li [class.first]="first" [class.done]="mission.state === 'complete'"
            [attr.data-index]="index" (click)="select(mission)">
            <strong [title]="mission | missionLabel:'Flight'">{{ mission.title }}</strong>
            <meter min="0" max="100" [value]="mission.progress">{{ mission.progress }}%</meter>
            <small>{{ mission.owner?.toLocaleUpperCase() || "unassigned" }}</small>
            @switch (mission.state) {
              @case ("queued") { <span class="queued">Queued</span> }
              @case ("active") { <span class="active">In flight</span> }
              @default { <span class="complete">Complete ✓</span> }
            }
            <button type="button" (click)="remove(mission.id, $event)">Remove</button>
          </li>
        } @empty {
          <li class="empty">No missions match “{{ query }}”.</li>
        }
      </ul>

      @defer (on viewport; prefetch on idle) {
        <aside class="details">Diagnostics ready for {{ selectedMission?.id }}</aside>
      } @placeholder (minimum 100ms) {
        <aside>Waiting for diagnostics…</aside>
      } @loading (after 50ms; minimum 100ms) {
        <aside>Loading details…</aside>
      } @error {
        <aside>Diagnostics unavailable.</aside>
      }

      <ng-container *ngIf="store.missions().length > 0">
        <ng-template #legend><span>queued · active · complete</span></ng-template>
      </ng-container>
    </main>
  `,
  styles: [
    `:host { display: block; color: #223; --accent: rebeccapurple; }
     :host(.compact) .missions { gap: .25rem; }
     .board { max-width: 60rem; margin-inline: auto; padding: clamp(1rem, 3vw, 2rem); }
     .toolbar { display: flex; gap: .75rem; align-items: end; }
     .missions { display: grid; gap: .75rem; padding: 0; list-style: none; }
     .missions li { border-inline-start: .3rem solid var(--accent); padding: .75rem;
       &.done { opacity: .7; }
       &:hover { background: color-mix(in srgb, var(--accent) 8%, white); }
     }
     meter { accent-color: var(--accent); }
     .error { color: #b00020; }
     @media (max-width: 40rem) { .toolbar { flex-direction: column; } }`,
  ],
})
export class MissionBoardComponent {
  readonly store = inject(MissionStore);
  readonly loading = signal(false);
  readonly error = signal<string | null>(null);
  readonly selected = signal<Mission | null>(null);
  readonly visible = computed(() => {
    const needle = this.query.trim().toLocaleLowerCase();
    return this.store.missions().filter(mission =>
      mission.title.toLocaleLowerCase().includes(needle) || mission.tags.some(tag => tag.includes(needle))
    );
  });

  @Input({ alias: "boardTitle" }) heading = "Orbital mission board";
  @Output() changed = new EventEmitter<readonly Mission[]>();
  @ViewChild("titleHeading") titleHeading?: ElementRef<HTMLHeadingElement>;
  @HostBinding("attr.role") readonly role = "region";
  query = "";
  compact = false;
  accent = "#7048e8";

  create(): void {
    const title = this.query.trim();
    if (!title) return;
    this.selected.set(this.store.add(title));
    this.query = "";
  }

  select(mission: Mission): void { this.selected.set(mission); }
  clearSelection(): void { this.selected.set(null); }
  noteFocus(element: HTMLElement): void { element.dataset["fixture"] = "focused"; }

  remove(id: MissionId, event: MouseEvent): void {
    event.stopPropagation();
    this.store.remove(id);
    if (this.selected()?.id === id) this.clearSelection();
  }
}
