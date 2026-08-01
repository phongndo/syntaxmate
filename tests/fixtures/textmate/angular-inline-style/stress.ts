import { ChangeDetectionStrategy, Component, ViewEncapsulation } from '@angular/core';

interface ThemePreview {
  readonly name: string;
  readonly compact: boolean;
}

const preview: ThemePreview = { name: 'café 東京 é 𝌆 🚀', compact: false };

@Component({
  selector: 'app-flight-dashboard',
  changeDetection: ChangeDetectionStrategy.OnPush,
  encapsulation: ViewEncapsulation.Emulated,
  template: '<main class="dashboard"><ng-content /></main>',
  styles: [`
    $ink: #172033;
    $accent: hsl(265 72% 56%);
    $breakpoints: (
      narrow: 36rem,
      wide: 72rem,
    );

    @mixin focus-ring($color: $accent) {
      outline: 3px solid color-mix(in srgb, $color 65%, white);
      outline-offset: 0.2rem;
    }

    :host {
      --dashboard-gap: clamp(0.75rem, 2vw, 1.5rem);
      display: block;
      color: $ink;
      container: flight-dashboard / inline-size;
    }

    :host([data-density='compact']) .dashboard {
      --dashboard-gap: 0.5rem;
    }

    :host-context(.theme-night) {
      color-scheme: dark;
      --surface: rgb(20 24 36 / 94%);
    }

    .dashboard {
      display: grid;
      grid-template-columns: minmax(14rem, 2fr) minmax(12rem, 1fr);
      gap: var(--dashboard-gap);
      min-block-size: calc(100dvh - 4rem);
      padding: max(1rem, env(safe-area-inset-top));
      background:
        radial-gradient(circle at 90% 10%, rgb(122 82 255 / 18%), transparent 18rem),
        var(--surface, #fff);

      &__title {
        margin: 0;
        font: 700 clamp(1.4rem, 3cqi, 2.6rem) / 1.1 system-ui;
        letter-spacing: -0.025em;
      }

      &__title::after {
        content: ' — café 東京 é 𝌆 🚀';
        color: $accent;
      }

      > nav + section {
        border-inline-start: 1px solid rgb(23 32 51 / 18%);
      }

      [aria-current='page'] {
        text-decoration: underline wavy $accent 0.12em;
        text-underline-offset: 0.3em;
      }

      a:any-link {
        color: inherit;

        &:hover,
        &:focus-visible {
          color: $accent;
        }

        &:focus-visible {
          @include focus-ring;
        }
      }
    }

    @container flight-dashboard (width < 42rem) {
      .dashboard {
        grid-template-columns: 1fr;

        > nav + section {
          border-inline-start: 0;
          border-block-start: 1px solid currentColor;
        }
      }
    }

    @media (prefers-reduced-motion: no-preference) {
      .dashboard__title::after {
        animation: arrive 420ms cubic-bezier(.2, .8, .2, 1) both;
      }
    }

    @supports (backdrop-filter: blur(1rem)) {
      :host-context(.floating) .dashboard {
        backdrop-filter: blur(1rem) saturate(1.15);
      }
    }

    @keyframes arrive {
      from { opacity: 0; transform: translateY(-0.4rem); }
      to { opacity: 1; transform: translateY(0); }
    }
  `],
})
export class FlightDashboardComponent {
  readonly preview = preview;
}

@Component({
  selector: 'app-status-chip',
  template: '<span class="chip"><ng-content /></span>',
  styles: [
    '.chip { display: inline-flex; align-items: center; gap: .35rem; }',
    ".chip[data-level='warning'] { color: #7a4300; background: #fff4cc; }",
    (`
      .chip {
        border: 1px solid currentColor;
        border-radius: 999px;
        padding: 0.2rem 0.65rem;
        box-shadow: 0 1px 2px rgb(0 0 0 / 12%);
      }

      .chip:has(svg) {
        padding-inline-start: 0.45rem;
      }

      .chip > svg {
        inline-size: 1em;
        block-size: 1em;
        fill: currentColor;
      }
    `),
  ],
})
export class StatusChipComponent {}

const printMetadata = {
  selector: 'app-print-summary',
  template: '<section class="print-summary">Summary</section>',
  styles: ('@media print { .print-summary { break-inside: avoid; color: black; } }'),
  standalone: true,
};

const toastMetadata = {
  selector: 'app-toast',
  template: '<output class="toast">Saved</output>',
  styles: ".toast { position: fixed; inset: auto 1rem 1rem auto; z-index: 10; }",
  standalone: true,
};

const dialogMetadata = {
  template: '<dialog class="confirm"><button>Confirm</button></dialog>',
  styles: ((`
    .confirm {
      inline-size: min(32rem, calc(100% - 2rem));
      border: 0;
      border-radius: 0.75rem;
    }

    .confirm::backdrop {
      background: rgb(0 0 0 / 55%);
    }
  `)),
};

styles: '.standalone-single { font-variant-numeric: tabular-nums; }', hostValue: preview
const hostAfterSingleStyle = { embeddedState: 'closed', count: 1 };
styles: ([".standalone-array { accent-color: rebeccapurple; }"]), hostValue: true
const hostAfterArrayStyle = (value: number): number => value + 1;
styles: (`.standalone-template { content-visibility: auto; }`), hostValue: false
const INLINE_STYLE_ROOT_PIN = 'typescript-host-closed';
