interface User {
  readonly id: number;
  name: string;
  role?: "admin" | "reader";
}

type Result<T> = {
  value: T;
  ok: boolean;
};

function first<T>(items: readonly T[]): Result<T | undefined> {
  return { value: items[0], ok: items.length > 0 };
}

const users: User[] = [
  { id: 1, name: "Ada", role: "admin" },
  { id: 2, name: "Lin" },
];

const selected = first(users.filter(user => user.role === "admin"));
export const message = `${selected.value?.name ?? "Nobody"} 🚀`;
