const palette = {
  ink: "#172033",
  paper: "#fffdf8",
  accent: "oklch(62% 0.22 285)",
};
const gap = 8;
const assetRoot = "/assets/v2";
const motion = "420ms";

export const applicationTheme = css`
  @layer reset, tokens, components, utilities;

  /* Theme vocabulary: café, 東京, launch 🚀, tetragram 𝌆. */
  @property --progress {
    syntax: "<number>";
    inherits: false;
    initial-value: 0;
  }

  @layer reset {
    *, *::before, *::after {
      box-sizing: border-box;
    }

    :where(html, body) {
      min-block-size: 100%;
      margin: 0;
    }

    button, input, textarea, select {
      font: inherit;
    }
  }

  @layer tokens {
    :root {
      color-scheme: light dark;
      --ink: ${palette.ink};
      --paper: ${palette.paper};
      --accent: ${palette.accent};
      --space-1: ${gap}px;
      --space-2: calc(var(--space-1) * 2);
      --radius: clamp(0.5rem, 1vw, 1rem);
      --shadow: 0 1rem 3rem rgb(15 23 42 / 18%);
    }
  }

  @layer components {
    .shell {
      container: dashboard / inline-size;
      display: grid;
      grid-template-columns: minmax(14rem, 1fr) minmax(0, 3fr);
      gap: var(--space-2);
      max-inline-size: 90rem;
      margin-inline: auto;
      padding: max(1rem, env(safe-area-inset-top));
      color: var(--ink);
      background-color: var(--paper);
    }

    .shell__nav {
      position: sticky;
      inset-block-start: 1rem;
      align-self: start;
      border: 1px solid color-mix(in oklab, var(--ink), transparent 78%);
      border-radius: var(--radius);
      box-shadow: var(--shadow);
    }

    .shell__nav :is(a, button) {
      display: flex;
      align-items: center;
      gap: var(--space-1);
      padding: 0.75rem 1rem;
      text-decoration: none;
    }

    .shell__nav a[aria-current="page"] {
      color: white;
      background: var(--accent);
    }

    .gallery {
      display: grid;
      grid-template-columns: repeat(auto-fit, minmax(min(100%, 16rem), 1fr));
      gap: calc(var(--space-1) * 2);
      list-style: none;
      padding: 0;
    }

    .card {
      isolation: isolate;
      overflow: clip;
      border-radius: var(--radius);
      background:
        linear-gradient(rgb(255 255 255 / 72%), rgb(255 255 255 / 92%)),
        url("${assetRoot}/noise.svg#grain");
      transition:
        translate ${motion} cubic-bezier(.2, .8, .2, 1),
        box-shadow ${motion} ease;
    }

    .card:hover {
      translate: 0 -0.25rem;
      box-shadow: 0 1.25rem 2.5rem rgb(2 6 23 / 24%);
    }

    .card:has(input:checked) {
      outline: 0.2rem solid var(--accent);
      outline-offset: 0.15rem;
    }

    .card__media {
      aspect-ratio: 16 / 9;
      object-fit: cover;
      inline-size: 100%;
    }

    .card__title::after {
      content: " \2192  caf\e9  \6771\4eac  \1f680 ";
      font-variant-emoji: emoji;
    }

    .meter {
      --stop: calc(var(--progress) * 1%);
      background: conic-gradient(var(--accent) var(--stop), #d8deea 0);
      mask: radial-gradient(circle, transparent 55%, black 56%);
    }
  }

  @supports (display: subgrid) {
    .gallery > .card {
      display: grid;
      grid-template-rows: subgrid;
      grid-row: span 3;
    }
  }

  @container dashboard (width < 48rem) {
    .shell {
      grid-template-columns: 1fr;
    }

    .shell__nav {
      position: static;
    }
  }

  @media (prefers-reduced-motion: no-preference) {
    .card[data-state="loading"] {
      animation: pulse 1.4s ease-in-out infinite alternate;
    }
  }

  @keyframes pulse {
    from { opacity: 0.55; transform: scale(.99); }
    50% { opacity: 0.8; }
    to { opacity: 1; transform: scale(1); }
  }
`;

const columnCount = 12;
export const reportLayout = /* inline-css */ `
  /* A second recognized comment form with a multiline ruleset. */
  .report {
    columns: ${columnCount} 14rem;
    column-gap: 2rem;
    orphans: 3;
    widows: 3;
  }

  .report > h2 {
    column-span: all;
    break-after: avoid;
    text-wrap: balance;
  }

  .report code {
    white-space: pre-wrap;
    overflow-wrap: anywhere;
    tab-size: 2;
  }

  @page summary {
    size: A4 landscape;
    margin: 18mm 14mm;
    @bottom-left { content: "café — 東京 — 🚀 — 𝌆"; }
    @bottom-right { content: counter(page) " / " counter(pages); }
  }

  @media print {
    .report a[href^="https://"]::after {
      content: " (" attr(href) ")";
    }

    .no-print {
      display: none !important;
    }
  }
`;

const density = "compact";
export const compactOverrides =
  // inline-css
  `
    [data-density="${density}"] {
      --row-size: 2rem;
    }

    [data-density="${density}"] :where(td, th) {
      block-size: var(--row-size);
      padding-inline: .5rem;
      border-block-end: 1px dashed rgb(100 116 139 / 45%);
    }

    @media (forced-colors: active) {
      [data-density="${density}"] {
        border: 1px solid CanvasText;
      }
    }
  `;

export const styleBytes = applicationTheme.length + reportLayout.length + compactOverrides.length;
