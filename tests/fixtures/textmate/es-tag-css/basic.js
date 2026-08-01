const accent = "#7c3aed";
const spacing = 12;

export const card = css`
  /* café 東京 🚀 𝌆 */
  .card:hover > .title {
    color: ${accent};
    margin-block: calc(${spacing}px * 2);
    background: linear-gradient(135deg, #fff, rgb(240 240 255 / 80%));
  }
`;

const label = "launch";
export const badge = /* inline-css */ `
  [data-label="${label}"]::before {
    content: "caf\e9  \6771\4eac  \1f680 ";
    display: inline-grid;
    border: 1px solid currentColor;
  }
`;

console.log(card.length + badge.length);
