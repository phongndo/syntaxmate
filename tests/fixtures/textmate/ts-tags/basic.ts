interface ViewModel {
  id: number;
  title: string;
  accent: string;
}

const view: ViewModel = { id: 42, title: "café 東京 λ 🚀 𝌆", accent: "#7c3aed" };

export const card = html`
  <article data-id="${view.id}"><h1>${view.title}</h1></article>
`;

export const theme = css`
  article { color: ${view.accent}; display: grid; }
`;

export const account = db.sql`
  SELECT id, display_name FROM account WHERE id = ${view.id};
`;

export const icon = xml`
  <svg xmlns="http://www.w3.org/2000/svg"><text>東京 🚀</text></svg>
`;

export const shader = glsl`void main() { gl_Position = vec4(${view.id}.0); }`;
export default { card, theme, account, icon, shader };
