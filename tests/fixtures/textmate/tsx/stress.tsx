import React from "react";

type Props = {
  title: string;
  items: string[];
};

type PanelState = "idle" | "loading" | "ready" | "failed";
type Identifier = string & { readonly __brand: unique symbol };
type Maybe<T> = T | null | undefined;
type Result<T, E = Error> = { ok: true; value: T } | { ok: false; error: E };
interface RecordLike {
  id: Identifier;
  label?: string;
  metadata: Readonly<Record<string, string | number>>;
}
interface ListProps<T extends RecordLike> {
  rows: readonly T[];
  selected?: T["id"];
  renderRow(row: T, index: number): React.ReactNode;
}

type EventName<T extends string> = `panel:${Lowercase<T>}`;
type Mutable<T> = { -readonly [K in keyof T]-?: T[K] };
type Payload<T> = T extends Result<infer Value, unknown> ? Value : never;
declare namespace Diagnostics {
  const enabled: boolean;
  function emit(name: EventName<string>, detail?: unknown): void;
}
const statuses = ["idle", "loading", "ready", "failed"] as const;
const statusLabels = {
  idle: "Waiting…",
  loading: "Chargement",
  ready: "準備完了",
  failed: "Échec",
} satisfies Record<PanelState, string>;
const identity = <T,>(value: T): T => value;
function tagged(parts: TemplateStringsArray, ...values: unknown[]): string {
  return parts.reduce((text, part, index) => `${text}${part}${values[index] ?? ""}`, "");
}

const escapedPath = String.raw`C:\fixtures\tsx\${"🚀"}`;
const announcement = tagged`Unicode ${"λ"} and astral ${"🛰️"}: ${escapedPath}`;

function parseRoute(input: string): Result<URL> {
  const routePattern = /^(?:\/[\p{L}\p{N}_~-]+)+\/?(?:\?[^#]*)?$/u;
  if (!routePattern.test(input)) {
    return { ok: false, error: new Error(`Invalid route: ${input}`) };
  }
  return { ok: true, value: new URL(input, "https://例え.example") };
}

function average(total: number, count: number, scale = 1): number {
  // These slashes are division, unlike /this slash-delimited expression/giu.
  const quotient = total / Math.max(count, 1) / scale;
  const looksLikeWord = /^(?:naïve|café|東京)$/iu.test(String(total));
  return looksLikeWord ? quotient / 2 : quotient;
}

function pickLabel(record: Maybe<RecordLike>): string {
  return (record?.label?.trim() || record?.metadata?.title?.toString()) ?? "(untitled)";
}
function DataList<T extends RecordLike>({ rows, selected, renderRow }: ListProps<T>) {
  return (
    <ol className="data-list" data-count={rows.length}>
      {rows.map((row, index) => (
        <li key={row.id} aria-current={row.id === selected ? "true" : undefined}>
          {renderRow(row, index)}
        </li>
      ))}
    </ol>
  );
}

export function StressPanel({ title, items }: Props) {
  const ratio = items.length / Math.max(title.length, 1);
  const hasCapital = /[A-Z][\w-]+/.test(title);

  return (
    <section data-title={title} data-state={hasCapital ? "open" : "closed"} aria-label={`Δ ${title}`}>
      {/* JSX comment with non-ASCII text: café λ🚀 */}
      <header className="panel__header">
        <h1>{title}</h1>
        <span data-ratio={ratio / 2}>{hasCapital ? "Regex" : "division"}</span>
      </header>
      <input disabled={ratio > 1} value={items.join(" / ")} readOnly />
      <ul>
        {items.map((item, index) => (
          <li key={`${item}-${index}`} data-index={index}>
            <button onClick={() => console.log(/ok\/(done)?/u.test(item))}>
              {item.toUpperCase()}
            </button>
          </li>
        ))}
      </ul>
    </section>
  );
}

type ToolbarProps = React.PropsWithChildren<{
  state: PanelState;
  onRefresh?: (event: React.MouseEvent<HTMLButtonElement>) => void;
}>;
function Toolbar({ state, onRefresh, children }: ToolbarProps) {
  const label = statusLabels[state];

  return (
    <>
      <nav
        aria-label="Primary — κύριο"
        className={`toolbar toolbar--${state}`}
        data-message={announcement}
      >
        <button type="button" onClick={onRefresh} disabled={state === "loading"}>
          {label}
        </button>
        {children ?? <span className="toolbar__empty">∅</span>}
      </nav>
      {/* A fragment sibling keeps the surrounding JSX state active. */}
      <output aria-live={state === "failed" ? "assertive" : "polite"}>{state}</output>
    </>
  );
}
const fixtureRows = [
  { id: "alpha" as Identifier, label: "Ångström", metadata: { rank: 1 } },
  { id: "rocket" as Identifier, label: "Launch 🚀", metadata: { rank: 2 } },
] as const satisfies readonly RecordLike[];

export const Fixture = () => {
  const [state, setState] = React.useState<PanelState>(statuses[0]);
  const parsed = React.useMemo(() => parseRoute("/résumé/東京?mode=tsx"), []);
  const detail = parsed.ok ? parsed.value.pathname : parsed.error.message;
  const metrics = { mean: average(42, fixtureRows.length), detail };

  React.useEffect(() => {
    Diagnostics.enabled && Diagnostics.emit("panel:ready", metrics);
    return () => void Diagnostics.emit("panel:idle");
  }, [metrics.detail, metrics.mean]);

  return (
    <main id="fixture-root" data-state={state} data-path={parsed.ok && parsed.value.href}>
        {/*
          Multiline JSX comment: braces {likeThis}, markup <not-a-tag />,
          BMP symbols Ω中 and astral text 🧪 remain comment content.
        */}
        <Toolbar
          state={state}
          onRefresh={(event) => {
            event.currentTarget.blur();
            setState((previous) => (previous === "loading" ? "ready" : "loading"));
          }}
        >
          <strong title={'Quotes " and \' survive'}>{identity(detail)}</strong>
        </Toolbar>

        <DataList
          rows={fixtureRows}
          selected={fixtureRows.at(0)?.id}
          renderRow={(row, index) => (
            <article data-index={index} data-json={JSON.stringify(row.metadata)}>
              <h2>{pickLabel(row)}</h2>
              <code>{`${row.id} :: ${row.metadata.rank ?? "—"}`}</code>
            </article>
          )}
        />

        {parsed.ok ? (
          <a href={parsed.value.href} target="_blank" rel="noreferrer">
            Open {parsed.value.hostname}
          </a>
        ) : (
          <p className="error">{parsed.error.message}</p>
        )}

        <svg viewBox="0 0 24 24" role="img" aria-labelledby="orbit-title">
          <title id="orbit-title">Orbit 🪐</title>
          <path d="M2 12c4-8 16-8 20 0s-16 8-20 0Z" fill="none" stroke="currentColor" />
        </svg>
    </main>
  );
};

export type { ListProps, Mutable, Payload };
