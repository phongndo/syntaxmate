const name = "café 東京 🚀 𝌆";
const count = 3;

const card = html`
  <article class="card" data-count="${count}">
    <!-- a complete HTML comment -->
    <h1>${name}</h1>
    <p title="Tom &amp; Jerry">Unicode: café 東京 🚀 𝌆</p>
    <input disabled value="${String(count)}">
  </article>
`;

const note = /* template */ `
  <aside aria-label="quoted &quot;value&quot;">
    <strong>Ready</strong><br>
  </aside>
`;

const hostText = `host resumed: ${card.length + note.length}`;
export { card, note, hostText };
