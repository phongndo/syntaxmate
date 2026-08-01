import { ChangeDetectionStrategy, Component, signal } from '@angular/core';
import { FormsModule, ReactiveFormsModule } from '@angular/forms';

interface Mission {
  readonly id: number;
  readonly name: string;
  readonly destination: string;
}

@Component({
  selector: 'app-mission-control',
  standalone: true,
  imports: [FormsModule, ReactiveFormsModule],
  changeDetection: ChangeDetectionStrategy.OnPush,
  template: (`
    <header class="masthead" data-city="東京">
      <a routerLink="/" aria-label="Mission home">🚀</a>
      <h1>{{ title() ?? 'café é 𝌆' }}</h1>
      <p [attr.lang]="locale">{{ subtitle | titlecase }}</p>
    </header>

    <main id="content">
      <!-- Angular bindings remain inside this fully closed backtick. -->
      @let selected = selectedMission();
      @if (selected; as mission) {
        <article
          class="mission-card"
          [class.is-urgent]="mission.priority > 7"
          [style.--progress.%]="mission.progress"
          [attr.data-mission-id]="mission.id"
        >
          <header>
            <h2>{{ mission.name }}</h2>
            <span title="destination">{{ mission.destination }}</span>
          </header>

          <dl>
            <div>
              <dt>Launch</dt>
              <dd>{{ mission.launchAt | date: 'medium' }}</dd>
            </div>
            <div>
              <dt>Crew</dt>
              <dd>{{ mission.crew?.length ?? 0 }}</dd>
            </div>
          </dl>

          @for (member of mission.crew; track member.id; let i = $index, first = $first) {
            <button
              type="button"
              [attr.aria-pressed]="member.id === activeCrewId"
              (click)="selectCrew(member, i)"
            >
              <img [src]="member.avatar" [alt]="member.name + ' portrait'" />
              <span>{{ i + 1 }}. {{ member.name | uppercase }}</span>
              @if (first) { <small>Commander</small> }
            </button>
          } @empty {
            <p role="status">No crew assigned.</p>
          }
        </article>
      } @else if (loading()) {
        <p class="skeleton" aria-live="polite">Loading mission…</p>
      } @else {
        <p>Select a mission from the queue.</p>
      }

      <section aria-labelledby="queue-heading">
        <h2 id="queue-heading">Queue</h2>
        <ul>
          <li *ngFor="let item of missions(); trackBy: trackMission">
            <a
              [routerLink]="['/missions', item.id]"
              routerLinkActive="active"
              #link="routerLinkActive"
            >
              {{ item.name }}
              <span [hidden]="!link.isActive">current</span>
            </a>
          </li>
        </ul>
      </section>

      <form [formGroup]="filters" (ngSubmit)="applyFilters()" novalidate>
        <label for="query">Search</label>
        <input
          id="query"
          name="query"
          type="search"
          formControlName="query"
          placeholder="café or 東京"
          (keydown.escape)="clearQuery()"
        />

        <label>
          Destination
          <select [(ngModel)]="destination" [ngModelOptions]="{ standalone: true }">
            <option value="">Everywhere</option>
            @for (place of destinations; track place.code) {
              <option [value]="place.code">{{ place.label }}</option>
            }
          </select>
        </label>

        <button type="submit" [disabled]="filters.invalid || loading()">
          Apply
        </button>
      </form>

      @switch (connectionState()) {
        @case ('online') {
          <output class="online">Telemetry online</output>
        }
        @case ('degraded') {
          <output class="degraded">Limited telemetry</output>
        }
        @default {
          <output class="offline">Telemetry unavailable</output>
        }
      }

      <ng-container *ngIf="helpVisible; else compactHelp">
        <aside aria-label="Keyboard help">
          Press <kbd>?</kbd> for shortcuts &amp; navigation.
        </aside>
      </ng-container>
      <ng-template #compactHelp let-label="label">
        <span>{{ label || 'Help' }}</span>
      </ng-template>

      <svg viewBox="0 0 120 24" role="img" aria-labelledby="orbit-title">
        <title id="orbit-title">Orbit path</title>
        <path [attr.d]="orbitPath" fill="none" stroke="currentColor" />
        <circle [attr.cx]="satelliteX()" cy="12" r="3" />
      </svg>
    </main>

    <footer>
      <small>Build {{ buildId }} · café 東京 é 𝌆 🚀</small>
      <ng-content select="[mission-footer]" />
    </footer>
  `),
  styles: ['.mission-card { display: grid; }'],
})
export class MissionControlComponent {
  readonly title = signal('Mission control');
  readonly selectedMission = signal<Mission | null>(null);
}

@Component({
  selector: 'app-inline-alert',
  standalone: true,
  template: '<p role="alert"><strong>{{ level }}</strong>: {{ message }}</p>',
})
export class InlineAlertComponent {}

const buttonMetadata = {
  selector: 'app-save-button',
  template: ("<button type='button' (click)='save()'>{{ label || 'Save' }}</button>"),
  standalone: true,
};

const emptyStateMetadata = {
  selector: 'app-empty-state',
  template: ((`
    <section class="empty" aria-labelledby="empty-title">
      <h2 id="empty-title">Nothing queued</h2>
      <p>{{ detail ?? 'Try another filter.' }}</p>
      <button (click)="reset.emit()">Reset filters</button>
    </section>
  `)),
  standalone: true,
};

template: '<span class="standalone-single">{{ value | number }}</span>', hostValue: true
const hostAfterSingleTemplate = { state: 'root', unicode: 'café 東京 é 𝌆 🚀' };
template: (("<em [title]='hint'>{{ hint }}</em>")), hostValue: false
const hostAfterDoubleTemplate = (input: string): string => input.trim();
template: (`<output>{{ completed ? 'done' : 'pending' }}</output>`), hostValue: 1
const INLINE_TEMPLATE_ROOT_PIN = 'typescript-host-closed';
