type BadgeProps = {
  label: string;
  count?: number;
};

const colors = ["red", "green", "blue"] as const;

export function Badge({ label, count = 0 }: BadgeProps) {
  const active = count > 0;
  return (
    <section className={active ? "badge active" : "badge"}>
      <h2>{label} 🚀</h2>
      <span aria-label="count">{count}</span>
      <ul>
        {colors.map(color => (
          <li key={color} style={{ color }}>
            {color.toUpperCase()}
          </li>
        ))}
      </ul>
    </section>
  );
}
