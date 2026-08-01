# Polar stress policy: broad grammar closure with Ελληνικά, 東京, and 🚀.
# Comments contain fake_rule(x) and "quoted text" without changing scopes.
type allow(actor: Actor, action: String, resource: Resource);
type has_role(actor: Actor, role: String, resource: Resource);
type has_permission(actor: Actor, permission: String, resource: Resource);
type related(subject: Resource, relation: String, object: Resource);

actor User {}
actor Robot extends User {}
actor ServiceAccount extends User, Robot {}

global {
  roles = ["member", "admin", "auditor"];
  permissions = ["create", "inspect", "debug"];
  "create" if "member";
  "inspect" if "auditor";
  "debug" if "admin";
  "member" if "admin";
}

resource Organization {
  roles = ["guest", "member", "owner"];
  permissions = ["view", "invite", "delete"];
  relations = { parent: Organization, founder: User };
  "view" if "guest";
  "guest" if "member";
  "invite" if "member";
  "member" if "owner";
  "delete" if "owner";
  "owner" if global "admin";
}

resource Repository {
  roles = ["reader", "contributor", "maintainer"];
  permissions = ["read", "write", "push", "archive"];
  relations = { parent: Organization, owner: User };
  "read" if "reader";
  "reader" if "contributor";
  "write" if "contributor";
  "contributor" if "maintainer";
  "push" if "maintainer";
  "archive" if "owner" on "parent";
}

resource Issue {
  roles = ["viewer", "editor", "closer"];
  permissions = ["read", "edit", "close"];
  relations = { repository: Repository, reporter: User };
  "read" if "viewer";
  "viewer" if "reader" on "repository";
  "edit" if "editor";
  "editor" if "contributor" on "repository";
  "close" if "closer";
  "closer" if "maintainer" on "repository";
}

resource Document extends Repository {
  roles = ["reviewer", "publisher"];
  permissions = ["review", "publish"];
  relations = { folder: Repository };
  "review" if "reviewer";
  "publish" if "publisher";
  "reviewer" if "reader" on "folder";
}

resource Dashboard {
  roles = ["observer", "operator"];
  permissions = ["read", "refresh"];
  relations = { organization: Organization };
  "read" if "observer";
  "refresh" if "operator";
  "operator" if "owner" on "organization";
}

allow(actor: Actor, action: String, resource: Resource) if
  has_permission(actor, action, resource) and
  not suspended(actor) and
  not quarantined(resource);

allow(actor: Actor, "read", resource: Repository) if
  public(resource) or has_role(actor, "reader", resource);

allow(actor: Actor, action, resource) if
  emergency(actor) and action in ["read", "inspect"] and not denied(resource);

has_permission(actor, permission, resource) if
  direct_permission(actor, permission, resource);

has_permission(actor, permission, resource) if
  inherited_permission(actor, permission, resource);

has_role(actor, role, resource) if assigned(actor, role, resource);
has_role(actor, "reader", repo: Repository) if public(repo);
has_role(actor, role, repo: Repository) if
  related(repo, "parent", org: Organization) and has_role(actor, role, org);

same_identity(left, right) if left = right;
different_identity(left, right) if left != right;
ordered(left, right) if left < right;
bounded(value, low, high) if value >= low and value <= high;
positive(value) if value > 0;
negative(value) if value < 0;
ratio(total, count, result) if result = total / count;
weighted(base, factor, result) if result = base * factor + 1;
difference(left, right, result) if result = left - right;

contains_role(role, roles) if role in roles;
matches_actor(value) if value matches User;
either_owner(actor, repo) if
  has_role(actor, "maintainer", repo) or has_relation(actor, "owner", repo);
all_visible(actor, resources) if
  forall(resource in resources, allow(actor, "read", resource));
descendant(child, parent) if child in descendants of parent;
trace(actor) if print(actor) and debug(actor) and cut;

unicode_profile(
  User { name: "Zoë 🚀", active: true },
  { city: "東京", greeting: "γειά", badges: ["naïve", "café"] }
);
numeric_samples(-42, +17, 0, 6.022e+23, -1.5e-2, 3.14159);
boolean_samples(true, false);
nested_samples([1, 2, [3, 4]], { left: (1 + 2), right: { ok: true } });
escaped_samples("line\\nquote: \" slash: \\", "emoji 🚀");
qualified_rule(App::Policy::check, Domain::User { id: 7 });

?= allow(User { name: "Ada" }, "read", Repository { id: 1 });
?= numeric_samples(-42, +17, 0, 6.022e+23, -1.5e-2, 3.14159);
?= contains_role("admin", ["member", "admin"]);

test "repository reader" {
  setup {
    fixture organization_graph;
    assigned(User { name: "Ada" }, "reader", Repository { id: 1 });
    public(Repository { id: 2 });
  }
  assert allow(User { name: "Ada" }, "read", Repository { id: 1 });
  assert allow(User { name: "李雷" }, "read", Repository { id: 2 });
  assert_not allow(User { name: "Ada" }, "archive", Repository { id: 1 });
}

test "organization hierarchy" {
  setup {
    fixture organization_graph;
    assigned(User { name: "Grace" }, "owner", Organization { id: 10 });
    related(Repository { id: 11 }, "parent", Organization { id: 10 });
  }
  assert has_role(User { name: "Grace" }, "owner", Organization { id: 10 });
  assert allow(User { name: "Grace" }, "archive", Repository { id: 11 });
  assert_not suspended(User { name: "Grace" });
}

test "numeric and collection terms" {
  setup {
    fixture numeric_graph;
    score(User { name: "Renée" }, 98.5);
  }
  assert bounded(98.5, 0, 100);
  assert contains_role("editor", ["viewer", "editor", "closer"]);
  assert_not negative(+17);
}

test "Unicode object literals 🚀" {
  setup {
    fixture unicode_graph;
    assigned(User { name: "宮沢" }, "maintainer", Repository { id: 東京 });
  }
  assert has_role(User { name: "宮沢" }, "maintainer", Repository { id: 東京 });
  assert allow(User { name: "宮沢" }, "push", Repository { id: 東京 });
  assert_not quarantined(Repository { id: 東京 });
}

test "iff query variables" {
  setup { fixture organization_graph; }
  assert allow(User { name: "Ada" }, action: String, Repository { id: 1 }) iff
    action in ["read"];
  assert_not allow(User { name: "Mallory" }, "delete", Organization { id: 10 });
}

# A final comment exercises keywords as plain comment text: of on global type.
?= boolean_samples(true, false);
