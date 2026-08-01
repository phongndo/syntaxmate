type MissionState = "queued" | "active" | "complete";

interface Mission {
  readonly id: number;
  name: string;
  state: MissionState;
  progress: number;
}

const locale = "ja-JP";
const accent = "#6d28d9";
const tenantId = 17;
const exposure = 1.25;
const missions: Mission[] = [
  { id: 1, name: "Café relay", state: "active", progress: 72 },
  { id: 2, name: "東京 survey 🚀", state: "queued", progress: 0 },
  { id: 3, name: "Lambda archive λ 𝌆", state: "complete", progress: 100 },
];
const database = { sql: String.raw };

export const dashboard = html`
  <!doctype html>
  <section class="dashboard" data-locale="${locale}">
    <!-- Every host interpolation closes before HTML parsing resumes. -->
    <header>
      <h1>Mission control: café 東京 λ 🚀 𝌆</h1>
      <output>${missions.filter(({ state }) => state === "active").length}</output>
    </header>
    <ol>
      ${missions.map((mission) => `<li data-state="${mission.state}">${mission.name}</li>`).join("")}
    </ol>
    <footer title="Tea &amp; telemetry">Ready</footer>
  </section>
`;
const afterDashboard: number = dashboard.length;

export const compactCard = inline-html`
  <article data-count="${missions.length}">
    <strong>${missions[0]?.name ?? "No mission"}</strong>
  </article>
`;

export const navigation = template`
  <nav aria-label="Mission pages">
    <a href="/missions?locale=${locale}">All missions</a>
  </nav>
`;

export const statusBadge = inline-template`
  <span class="status" data-state="${missions[1].state}">Queued</span>
`;

export const commentedHtml = /* html */ `
  <aside><p>Block-selected HTML &amp; closed markup.</p></aside>
`;

export const lineSelectedHtml =
  // inline-html
  `<button type="button" data-id="${missions[0].id}">Open</button>`;
const afterHtml: boolean = lineSelectedHtml.includes("button");

export const applicationCss = css`
  :root {
    --accent: ${accent};
    --space: 0.75rem;
  }
  .dashboard {
    display: grid;
    gap: var(--space);
    color: color-mix(in oklab, var(--accent), black 20%);
  }
  .dashboard:has([data-state="active"]) {
    border-inline-start: 0.25rem solid var(--accent);
  }
  @media (width < 48rem) {
    .dashboard { grid-template-columns: 1fr; }
  }
`;

export const compactCss = inline-css`
  [data-state="${missions[0].state}"] { font-weight: 700; }
`;

export const printCss = /* inline-css */ `
  @media print {
    .dashboard::after { content: "café 東京 λ 🚀 𝌆"; }
  }
`;

export const lineSelectedCss =
  // css
  `.status { color: ${accent}; text-decoration: underline; }`;
const afterCss = applicationCss.length + lineSelectedCss.length;

export const missionQuery = database.sql`
  SELECT
    id,
    name,
    state,
    progress
  FROM mission
  WHERE tenant_id = ${tenantId}
    AND state IN (${"active"}, ${"queued"})
  ORDER BY progress DESC, name COLLATE "C";
`;

export const auditQuery = sql`
  INSERT INTO audit_event (tenant_id, message)
  VALUES (${tenantId}, 'opened café 東京 λ 🚀 𝌆 dashboard')
  RETURNING id, created_at;
`;

export const schemaQuery = /* inline-sql */ `
  CREATE TABLE IF NOT EXISTS mission_note (
    id BIGINT PRIMARY KEY,
    mission_id BIGINT NOT NULL,
    body TEXT NOT NULL
  );
`;

export const lineSelectedSql =
  // sql
  `UPDATE mission SET progress = ${missions[0].progress} WHERE id = ${missions[0].id};`;
const afterSql: readonly string[] = [missionQuery, auditQuery, schemaQuery, lineSelectedSql];

export const catalogXml = xml`<?xml version="1.0" encoding="UTF-8"?>
  <catalog xmlns="urn:example:missions">
    <mission id="one"><name>Café relay</name><state>active</state></mission>
    <mission id="two"><name>東京 survey 🚀</name><state>queued</state></mission>
  </catalog>
`;

export const feedXml = inline-xml`
  <feed xmlns="urn:example:feed"><title>Lambda archive λ 𝌆</title></feed>
`;

export const rocketSvg = /* svg */ `
  <svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 32 32" role="img">
    <title>Rocket 🚀</title>
    <circle cx="16" cy="16" r="14" fill="#6d28d9" />
    <path d="M10 22 L16 6 L22 22 Z" fill="white" />
  </svg>
`;

export const compactSvg = /* inline-svg */ `
  <svg xmlns="http://www.w3.org/2000/svg"><path d="M0 0h8v8H0z" /></svg>
`;

export const commentedXml = /* xml */ `
  <svg xmlns="http://www.w3.org/2000/svg"><text>café 東京</text></svg>
`;

export const lineSelectedXml =
  // inline-xml
  `<message xmlns="urn:example:message"><body>Closed XML 𝌆</body></message>`;
const afterXml = [catalogXml, feedXml, rocketSvg, compactSvg, commentedXml, lineSelectedXml];

export const vertexShader = glsl`
  #version 300 es
  precision highp float;
  layout(location = 0) in vec3 a_position;
  uniform mat4 u_projection;
  uniform float u_exposure;
  void main() {
    vec3 adjusted = a_position * ${exposure};
    gl_Position = u_projection * vec4(adjusted, 1.0);
  }
`;

export const fragmentShader = inline-glsl`
  #version 300 es
  precision highp float;
  out vec4 outColor;
  void main() {
    outColor = vec4(${missions[0].progress}.0 / 100.0, 0.4, 0.8, 1.0);
  }
`;

export const computeShader = /* inline-glsl */ `
  #version 310 es
  layout(local_size_x = 8) in;
  void main() { uint index = gl_GlobalInvocationID.x; }
`;

export const lineSelectedShader =
  // glsl
  `precision mediump float;
   void main() { gl_FragColor = vec4(${exposure}); }`;
const afterGlsl = vertexShader.length + fragmentShader.length + computeShader.length;

function summarize(): Record<string, number | boolean> {
  return {
    afterDashboard,
    afterHtml,
    afterCss,
    sqlDocuments: afterSql.length,
    xmlDocuments: afterXml.length,
    afterGlsl,
  };
}

export default summarize;
