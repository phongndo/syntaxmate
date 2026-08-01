import defaultFormatter, { format as formatValue, version as libraryVersion } from "./formatter.js";
import * as unicodeData from "./unicode-data.js";
export { default as helper } from "./helper.js";
/* JavaScript stress fixture.
 * Multi-line comment with non-ASCII text: café λ🚀.
 */

const path = "/api/v1/café";
const routePattern = /^\/api\/(?<version>v\d+)\/[\p{Letter}-]+$/u;
const attributePattern = /(\w+)\s*=\s*(["']).*?\2/g;

const total = 84;
const count = 6;
const ratio = total / count / 2;
const divideByRegexSourceLength = total / /[0-9]+/.source.length;

const replacement = `multi-line template
route=${path}
match=${routePattern.test(path) ? "✓" : "✗"}`;

function normalize(value) {
  return value.replace(attributePattern, "$1=…").trim() / 2;
}

export { routePattern, ratio, divideByRegexSourceLength, normalize, replacement };
const binaryMask = 0b1010_0110;
const octalMode = 0o755;
const hexColor = 0xff_88_cc;
const preciseCount = 1_000_000.25e-2;
const hugeSequence = 9_007_199_254_740_993n;
const signedBigInt = -42n;
const words = "quoted\ntext";
const rawPath = String.raw`C:\temp\${"資料"}\file.txt`;
const unicodeMessage = "Καλημέρα, 世界 🌍";
const multiline = `first line
second line: ${unicodeMessage.toUpperCase()}
computed: ${(() => {
  const points = [3, 5, 8];
  return points.reduce((sum, point) => sum + point, 0);
})()}`;
const escapedSlash = /https?:\/\/(?:www\.)?example\.com\/[\w./?=&%-]*/gi;
const unicodeWord = /\b[\p{Script=Greek}\p{Script=Han}]+\b/gu;
const astralSymbols = /[😀-🙏]/gu;
const stickyToken = /\s*(?<kind>[A-Za-z_$][\w$]*|\d+(?:\.\d+)?)/y;
const lookAround = /(?<=prefix-)[a-z]+(?=-suffix)/i;
const literalBrackets = /[()[\]{}\\/]/g;
export class RecordStore extends Map {
  static category = "records";
  static #instances = 0;
  #revision = 0;
  #metadata;
  constructor(entries = [], { source = "memory", ...metadata } = {}) {
    super(entries);
    this.source = source;
    this.#metadata = { createdAt: Date.now(), ...metadata };
    RecordStore.#instances += 1;
  }
  static get instanceCount() {
    return this.#instances;
  }
  get revision() {
    return this.#revision;
  }
  get description() {
    return `${this.source}:${this.size}@${this.#revision}`;
  }
  set description(value) {
    this.#metadata.label = String(value ?? "untitled");
  }
  add(key, value = null) {
    this.set(key, { value, revision: ++this.#revision });
    return this;
  }
  *valuesByRevision(minimum = 0) {
    for (const [key, entry] of this) {
      if (entry.revision >= minimum) yield [key, entry.value];
    }
  }
  async *load(stream, { signal } = {}) {
    for await (const item of stream) {
      if (signal?.aborted) return;
      const { id, payload: value = {}, ...details } = item;
      yield this.add(id, { ...value, details });
    }
  }
}
const store = new RecordStore([["seed", { value: 1, revision: 0 }]], {
  source: "fixture",
  locale: "fr-FR",
});
store.description = null;
store.add("alpha", { enabled: true });
const settings = {
  theme: "dark",
  retries: 2,
  [unicodeMessage]: true,
  method(prefix = "item") {
    return `${prefix}-${this.retries}`;
  },
  async refresh(...sources) {
    return Promise.all(sources.map((source) => source?.reload?.()));
  },
  get label() {
    return this._label ?? this.theme;
  },
  set label(next) {
    this._label = next || "default";
  },
};
let runtimeOptions;
runtimeOptions ??= {};
runtimeOptions.timeout ??= 5_000;
runtimeOptions.verbose ||= false;
runtimeOptions.cache &&= new Map();
const coordinates = [12, 34, 56, 78];
const [x, y = 0, ...remainingCoordinates] = coordinates;
const { theme: activeTheme, missing = "fallback", ...otherSettings } = settings;
const mergedState = { ...otherSettings, activeTheme, missing, coordinates: [...coordinates, 90] };
void mergedState;
export async function fetchRecords(url, {
  headers: { authorization = "anonymous", ...headers } = {},
  retries = 1,
  transform = (value) => value,
} = {}) {
  let attempt = 0;
  retry: while (attempt <= retries) {
    try {
      const response = await fetch(url, { headers: { authorization, ...headers } });
      if (!response.ok) throw new Error(`HTTP ${response.status}`);
      const data = await response.json();
      return data?.items?.map(transform) ?? [];
    } catch (error) {
      if (error instanceof TypeError && attempt++ < retries) continue retry;
      throw new AggregateError([error], "Unable to fetch records", { cause: error });
    } finally {
      runtimeOptions.lastAttempt = attempt;
    }
  }
  return [];
}
function* tokenize(input) {
  stickyToken.lastIndex = 0;
  while (stickyToken.lastIndex < input.length) {
    const match = stickyToken.exec(input);
    if (!match) break;
    yield { token: match[0].trim(), kind: match.groups?.kind };
  }
}
async function selectAdapter(name) {
  const module = await import(`./adapters/${encodeURIComponent(name)}.js`);
  return module.default ?? module.adapter;
}
function classify(value) {
  switch (typeof value) {
    case "bigint":
      return value > 0n ? "positive bigint" : "non-positive bigint";
    case "string":
      return value.length ? "text" : "empty";
    case "object": {
      if (value === null) return "null";
      return Array.isArray(value) ? "array" : "object";
    }
    default:
      return "other";
  }
}
for (let index = 0; index < coordinates.length; index += 1) {
  if (coordinates[index] % 2 !== 0) continue;
  store.add(`point-${index}`, coordinates[index]);
}
for (const token of tokenize("alpha 42 beta")) {
  settings[token.kind] = token.token;
}
const uniqueKinds = new Set([...tokenize("one 2 two")].map(({ kind }) => classify(kind)));
const lookup = new Map(Array.from(uniqueKinds, (kind, index) => [kind, index]));
do {
  runtimeOptions.timeout -= 1;
} while (runtimeOptions.timeout > 4_995);
const html = (strings, ...values) => strings.reduce(
  (output, string, index) => output + string + (values[index] ?? ""),
  "",
);
const panel = html`<section lang="日本語">
  <h1>${defaultFormatter(formatValue(libraryVersion))}</h1>
  <p>${unicodeData?.labels?.welcome ?? "ようこそ 🚀"}</p>
</section>`;
export default Object.freeze({
  store,
  settings,
  panel,
  rawPath,
  multiline,
  patterns: { escapedSlash, unicodeWord, astralSymbols, lookAround, literalBrackets },
  numbers: { binaryMask, octalMode, hexColor, preciseCount, hugeSequence, signedBigInt },
  selectAdapter,
  coordinates: { x, y, remainingCoordinates },
  lookup,
  words,
});
