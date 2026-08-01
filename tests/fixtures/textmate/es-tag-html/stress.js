const locale = "ja-JP";
const title = "Café dashboard 東京 🚀 𝌆";
const user = { id: 42, name: "Zoë", active: true };
const items = ["alpha", "bravo", "東京"];
const formatCount = (value) => new Intl.NumberFormat(locale).format(value);

const documentShell = html`
  <!doctype html>
  <html lang="en">
    <head>
      <meta charset="utf-8">
      <meta name="viewport" content="width=device-width, initial-scale=1">
      <title>${title}</title>
      <link rel="stylesheet" href="/assets/dashboard.css">
      <meta name="theme-color" content="#172033">
      <meta property="og:type" content="website">
      <meta property="og:locale" content="${locale}">
      <meta property="og:title" content="${title}">
    </head>
    <body data-user-id="${user.id}">
      <!-- navigation remains wholly inside the template -->
      <header class="site-header">
        <h1>${title}</h1>
        <nav aria-label="Primary">
          <a href="/home?lang=${locale}">Home</a>
          <a href="/about" title="Café &amp; tea">About</a>
        </nav>
      </header>
      <main id="content">
        <p>Hello, <strong>${user.name}</strong></p>
        <p class="badge">café 東京 🚀 𝌆</p>
      </main>
      <section data-controller="dashboard">
        <output name="ready">ready</output>
        <span data-state="active">interactive host</span>
      </section>
    </body>
  </html>
`;
const afterDocument = documentShell.length;

const inventory = template`
  <section class="inventory" aria-labelledby="inventory-title">
    <h2 id="inventory-title">Inventory</h2>
    <ol start="1">
      <li data-key="${items[0]}">${items[0]}</li>
      <li data-key="${items[1]}">${items[1]}</li>
      <li data-key="${items[2]}">${items[2]}</li>
    </ol>
    <details open>
      <summary>Totals</summary>
      <dl>
        <dt>Visible</dt>
        <dd>${formatCount(items.length)}</dd>
        <dt>Owner</dt>
        <dd>${user.name}</dd>
      </dl>
    </details>
  </section>
`;
const afterInventory = { count: items.length, valid: inventory.length > 0 };

const profile = inline-html`
  <article class="profile" data-active="${user.active}">
    <header>
      <img src="/avatars/${user.id}.png" alt="Portrait of ${user.name}">
      <h2>${user.name}</h2>
    </header>
    <blockquote cite="https://example.test/quotes/42">
      <p>Use &lt;semantic&gt; markup &amp; keep it readable.</p>
    </blockquote>
    <form action="/users/${user.id}" method="post">
      <label for="display-name">Display name</label>
      <input id="display-name" name="displayName" value="${user.name}">
      <label>
        <input type="checkbox" name="active" checked>
        Active account
      </label>
      <button type="submit">Save</button>
    </form>
  </article>
`;
const afterProfile = profile.trim().length > 0;

const alertBox = /* html */ `
  <div class="alert" role="alert">
    <!-- entities exercise HTML escapes without ending the host template -->
    <p title="double &quot; single &#39; ampersand &amp;">
      Limits: 5 &lt; 8 &gt; 3; rocket: &#x1F680;.
    </p>
    <button type="button" aria-label="Dismiss">×</button>
  </div>
`;
const afterAlert = alertBox.includes("role=\"alert\"");

const tableView = /* inline-template */ `
  <table>
    <caption>${title}</caption>
    <thead>
      <tr>
        <th scope="col">Name</th>
        <th scope="col">Value</th>
      </tr>
    </thead>
    <tbody>
      <tr>
        <th scope="row">Count</th>
        <td>${formatCount(1200345)}</td>
      </tr>
      <tr>
        <th scope="row">Locale</th>
        <td><code>${locale}</code></td>
      </tr>
    </tbody>
  </table>
`;
const afterTable = [tableView, afterAlert].every(Boolean);

const commentedPanel =
  // template
  `<section class="comment-tag">
    <h2>Comment-selected template</h2>
    <p>Line marker form with ${items.length} substitutions.</p>
    <hr>
    <p>Host parsing should resume after the closing backtick.</p>
  </section>`;
const afterCommentedPanel = commentedPanel.length;

const inlineCommentPanel =
  // inline-html
  `<aside class="tip" data-owner="${user.id}">
    <!-- multiline comment
         with café 東京 🚀 𝌆
         and a safe closing marker -->
    <h3>Tip</h3>
    <p>Prefer <kbd>Ctrl</kbd> + <kbd>K</kbd></p>
  </aside>`;
const afterInlineComment = inlineCommentPanel.toUpperCase();

const media = html`
  <figure>
    <picture>
      <source media="(min-width: 800px)" srcset="/wide.webp 2x">
      <img src="/small.webp" alt="東京 skyline" loading="lazy">
    </picture>
    <figcaption>${title}</figcaption>
  </figure>
  <audio controls preload="metadata">
    <source src="/sound.ogg" type="audio/ogg">
    Your browser does not support audio.
  </audio>
`;
const afterMedia = media.replace(/\s+/g, " ").trim();

const component = inline-html`
  <custom-card data-index="${items.length}">
    <span slot="title">${title}</span>
    <template shadowrootmode="open">
      <span class="host-style">display block</span>
      <slot name="title">Fallback</slot>
    </template>
    <svg viewBox="0 0 20 20" aria-hidden="true">
      <circle cx="10" cy="10" r="8"></circle>
      <path d="M5 10h10"></path>
    </svg>
  </custom-card>
`;
const afterComponent = Boolean(component && afterMedia);

function summarizeHtml() {
  const lengths = [afterDocument, afterCommentedPanel, component.length];
  const total = lengths.reduce((sum, value) => sum + value, 0);
  return { total, afterInventory, afterProfile, afterTable, afterInlineComment };
}

export { documentShell, inventory, profile, alertBox, tableView, media, component };
export default summarizeHtml;
