const users = [
  { name: "Ada", active: true },
  { name: "Lin", active: false },
];

function label({ name, active }) {
  const state = active ? "ready" : "paused";
  return `${name}: ${state} 🚀`;
}

class Registry {
  #items = new Map();

  add(user) {
    this.#items.set(user.name, label(user));
  }
}

const registry = new Registry();
users.filter(({ active }) => active).forEach(user => registry.add(user));
export { Registry, label, registry };
