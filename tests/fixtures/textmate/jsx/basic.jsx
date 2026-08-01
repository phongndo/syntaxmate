/* Basic JSX fixture: café λ 🚀 𝌆 */
const crew = [{ id: 1, name: "Ada" }, { id: 2, name: "Lin" }];

export function LaunchCard({ ready = true }) {
  const heading = `Mission café
${ready ? "is ready 🚀" : "is waiting λ"}`;
  const visible = crew.filter(({ id }) => id > 0);

  return (
    <section className="launch-card" data-ready={ready}>
      <header>
        <h1>{heading}</h1>
        {/* Nested elements and expressions remain balanced. */}
      </header>
      <ul aria-label="Crew 𝌆">
        {visible.map((member) => (
          <li key={member.id}><strong>{member.name}</strong></li>
        ))}
      </ul>
      {ready ? <button type="button">Launch</button> : <span>Stand by</span>}
    </section>
  );
}
