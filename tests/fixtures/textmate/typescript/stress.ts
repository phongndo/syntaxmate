/* TypeScript stress fixture.
 * The comment spans multiple lines and contains non-ASCII: café λ🚀.
 */

type Pair<T extends string | number> = {
  left: T;
  right?: T;
};

const route = "/api/v1/café";
const routePattern = /^\/api\/(?<version>v\d+)\/[\p{Letter}-]+$/u;
const quotedAttribute = /(\w+)\s*=\s*(["']).*?\2/g;

const total = 42;
const count = 7;
const ratio = total / count / 2;
const regexThenDivision = routePattern.test(route) ? total / /\d+/.source.length : 0;

const message = `multi-line template
route=${route}
match=${routePattern.test(route) ? "✓" : "✗"}`;

export function pick<T extends string | number>(pair: Pair<T>): T {
  return pair.right ?? pair.left;
}

export const summary = `${pick({ left: "λ", right: "🚀" })}: ${message} ${ratio}`;

interface Identified<Id extends PropertyKey = string> {
  readonly id: Id;
  displayName: string;
  metadata?: Record<string, unknown>;
}

interface Timestamped {
  createdAt: Date;
  updatedAt: Date | null;
}

type Entity<Id extends PropertyKey = string> = Identified<Id> & Timestamped;
type MaybePromise<T> = T | PromiseLike<T>;
type ElementOf<T> = T extends readonly (infer Item)[] ? Item : never;
type JsonScalar = string | number | boolean | null;
type JsonValue = JsonScalar | { [key: string]: JsonValue } | JsonValue[];
type EventName<T extends string> = `on${Capitalize<T>}`;
type RouteParameters<Path extends string> =
  Path extends `${string}:${infer Param}/${infer Rest}`
    ? Param | RouteParameters<Rest>
    : Path extends `${string}:${infer Param}` ? Param : never;

type MutablePatch<T> = {
  -readonly [Key in keyof T]?: T[Key] extends Date ? Date | string : T[Key];
};

type HandlerMap<Events extends Record<string, unknown>> = {
  [Name in keyof Events as EventName<Name & string>]: (event: Events[Name]) => void;
};

type ApiPath = "/users/:userId/posts/:postId";
type ApiParameter = RouteParameters<ApiPath>;
type Coordinates = readonly [latitude: number, longitude: number, label?: string];
type WithStatus<T extends unknown[]> = [...values: T, status: "ok" | "error"];

const origin: Coordinates = [51.5072, -0.1276, "Londýn"];
const translations = {
  greeting: "こんにちは",
  farewell: "До свидания",
  launch: "Ready 🛰️",
} as const satisfies Record<string, string>;

function decode(input: string): JsonValue;
function decode(input: Uint8Array, encoding?: "utf-8" | "utf-16le"): JsonValue;
function decode(input: string | Uint8Array, encoding = "utf-8"): JsonValue {
  const text = typeof input === "string" ? input : new TextDecoder(encoding).decode(input);
  return JSON.parse(text) as JsonValue;
}

const registryTag: unique symbol = Symbol("registry.tag");

class Registry<T extends Identified> implements Iterable<T> {
  static readonly defaultCapacity = 32;
  readonly [registryTag] = true;
  #items = new Map<T["id"], T>();

  constructor(public readonly label: string, initial: Iterable<T> = []) {
    for (const item of initial) this.add(item);
  }

  get size(): number {
    return this.#items.size;
  }

  add<Item extends T>(item: Item): this {
    this.#items.set(item.id, item);
    return this;
  }

  get(id: T["id"]): T | undefined {
    return this.#items.get(id);
  }

  select<Selected extends T>(guard: (item: T) => item is Selected): Selected[];
  select<Result>(project: (item: T) => Result): Result[];
  select<Result>(project: (item: T) => Result): Result[] {
    return [...this.#items.values()].map(project);
  }

  *[Symbol.iterator](): Iterator<T> {
    yield* this.#items.values();
  }

  async *stream(signal?: AbortSignal): AsyncGenerator<T, void, void> {
    for (const item of this) {
      if (signal?.aborted) return;
      await Promise.resolve();
      yield item;
    }
  }
}

namespace Geometry {
  export interface Point {
    x: number;
    y: number;
  }

  export type Axis = keyof Point;

  export class Vector implements Point {
    constructor(public x = 0, public y = 0) {}

    scale(factor: number): Vector {
      return new Vector(this.x * factor, this.y * factor);
    }
  }

  export function distance(a: Point, b: Point = new Vector()): number {
    return Math.hypot(a.x - b.x, a.y - b.y);
  }

  export namespace Format {
    export const compact = ({ x, y }: Point): `${number},${number}` => `${x},${y}`;
  }
}

const protocolPattern = /^(?<scheme>https?):\/\/(?<host>[^/?#]+)(?<path>\/[^?#]*)?/iu;
const markdownLink = /\[(?<label>[^\]]+)]\((?<target>[^\s)]+)(?:\s+"[^"]*")?\)/g;

function scoreDocument(source: string, weight: number): number {
  const words = source.match(/[\p{L}\p{N}_]+/gu) ?? [];
  const density = words.length / Math.max(source.length, 1);
  const normalized = weight / 2 / (density || 1);
  return /^\s*(?:#|\/\/)/.test(source) ? normalized / 2 : normalized;
}

const releaseNotes = `## Release 🚀

The parser keeps **regex literals** such as ${protocolPattern.source}
separate from division like ${total} / ${count} = ${total / count}.

Nested interpolation: ${`coordinates ${Geometry.Format.compact({ x: origin[0], y: origin[1] })}`}
Escaped delimiters remain text: \`template\` and \${notAnInterpolation}.
`;

/* A second multiline comment exercises punctuation that resembles syntax:
 * generic-ish <T extends U>, optional ?. access, nullish ?? fallback,
 * and closing-looking fragments * / that are separated on purpose.
 * Unicode remains ordinary source text: naïve façade, 東京, and fox 🦊.
 */

async function* paginate<T>(
  load: (cursor: string | undefined) => Promise<readonly [T[], string?]>,
): AsyncGenerator<T, void, void> {
  let cursor: string | undefined;
  do {
    const [items, next] = await load(cursor);
    for (const item of items) yield item;
    cursor = next;
  } while (cursor !== undefined);
}

export { Geometry, Registry, decode, paginate, scoreDocument };
export type { ApiParameter, Entity, HandlerMap, JsonValue, MutablePatch, WithStatus };
