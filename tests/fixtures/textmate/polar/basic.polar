# Compact Polar policy with café, 東京, and astral 🚀 data.
type allow(actor: Actor, action: String, resource: Resource);
actor User {}
resource Repository {
  roles = ["reader", "maintainer"];
  permissions = ["read", "push"];
  relations = { parent: Organization };
  "read" if "reader";
  "push" if "maintainer";
}
global {
  roles = ["admin"];
  permissions = ["create"];
  "create" if "admin";
}
allow(user: User, action, repo: Repository) if
  has_permission(user, action, repo) and not blocked(user);
profile(User { name: "Zoë 🚀", active: true }, { city: "東京", score: 4.2e+1 });
member(name, ["café", "東京", "orbit"]);
signed_values(-7, +8, 0, 3.14);
escaped("quote: \" and slash: \\");
?= allow(User { name: "Ada" }, "read", Repository { id: 42 });
test "basic permission" {
  setup { fixture basic_graph; }
  assert allow(User { name: "Ada" }, "read", Repository { id: 42 });
  assert_not blocked(User { name: "Ada" });
}
