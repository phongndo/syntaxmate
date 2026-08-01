// Tier-B TypeSpec stress fixture.
// Unicode matrix: café λ 東京 🚀 𝌆
import "@typespec/http";
import "@typespec/rest";
using TypeSpec.Http;
using TypeSpec.Rest;
#suppress "deprecated" "fixture coverage"

namespace Fixture.Stress {
  /**
   * A generic response used throughout the fixture.
   * @template T response body
   * @prop data wrapped data
   * @returns a closed response model
   * @example ignored-but-scoped tag
   */
  @doc("Response for ${T}")
  model Response<T extends string = string> {
    @doc("HTTP status")
    status: int32 = 200;
    data: T;
    message?: "café λ 東京 🚀 𝌆";
    headers: {
      requestId: string;
      elapsed?: int64;
    };
  }

  /* A multiline block comment keeps state
     through punctuation { < @ and Unicode 東京 🚀
     before closing normally. */
  scalar Identifier extends string {
    init fromString(value: string);
    init fromInt(value: int64);
  }

  scalar Timestamp extends utcDateTime;
  scalar Score<T extends numeric = int32> extends numeric {
    init fromNumber(value: numeric);
  }

  @doc("Lifecycle values")
  enum Lifecycle {
    @doc("new value")
    created,
    active: "active",
    paused,
    `in-flight`: "in-flight",
    archived: "archived",
  }

  enum HttpCode {
    ok: 200,
    created: 201,
    noContent: 204,
    badRequest: 400,
    missing: 404,
    serverError: 500,
  }

  model AuditFields {
    createdAt: Timestamp;
    updatedAt?: Timestamp;
    revision: int64;
  }

  model Entity<TId extends Identifier = Identifier> extends AuditFields {
    id: TId;
    kind: string;
  }

  @doc("User record")
  model User is Entity<Identifier> {
    displayName: string;
    "content-type": "application/json";
    enabled: true;
    aliases: string[];
    coordinates: [float64, float64];
    metadata: Record<unknown>;
  }

  model Admin extends User {
    permissions: Permission[];
    level: 0 | 1 | 2 | 3;
  }

  model Page<T> {
    items: T[];
    next?: string;
    count: int32;
    ...AuditFields;
  }

  model SearchOptions {
    query?: string;
    limit?: int32 = 25;
    offset?: int32 = 0;
    states?: Lifecycle[];
  }

  union Permission {
    read: "read",
    write: "write",
    delete: "delete",
    wildcard: "*",
  }

  union Payload {
    user: User,
    admin: Admin,
    page: Page<User>,
    empty: null,
    literal: #{ locale: "fr-FR", label: "café" },
  }

  union Outcome<T> {
    success: Response<T>,
    failure: {
      code: HttpCode;
      reason: string;
    },
  }

  interface Reader<T> {
    @doc("Read one value")
    op read(id: Identifier): T;
    list(options?: SearchOptions): Page<T>;
  }

  interface Writer<T> extends Reader<T> {
    create(value: T): Response<T>;
    replace(id: Identifier, value: T): Response<T>;
    remove(id: Identifier): void;
  }

  @route("/users")
  interface Users extends Writer<User> {
    @get
    search(...SearchOptions): Page<User>;
    @post
    op promote(id: Identifier, permissions: Permission[]): Admin;
  }

  alias Name = string;
  alias StringResponse<T extends string = Name> = Response<T>;
  alias Pair = [string, int32];
  alias Shape = {
    width: float64;
    height: float64;
  };

  const truth: boolean = true;
  const falsity = false;
  const decimal = 1_024;
  const negativeExponent = 6.022e-23;
  const hexadecimal = 0xCA_FE;
  const binary = 0b1010_0110;
  const bigint = 9007199254740991n;
  const tupleValue = #["north", 42, true];
  const objectValue = #{
    title: "東京",
    rocket: "🚀",
    escaped: "quote: \" slash: \\",
    nested: #{ symbol: "𝌆", lambda: "λ" },
  };

  const computed = buildName("café", 2);
  const typedObject: valueof Shape = #{ width: 10.5, height: 20.25 };
  alias ShapeType = typeof typedObject;
  alias Specialized = Response<T = string>;

  const report = """
    Report for ${computed}
    Unicode remains café λ 東京 🚀 𝌆.
    Braces in text { are harmless } and escapes work: \".
    The triple-quoted state closes on the next delimiter.
  """;

  extern dec sealed(target: unknown);
  extern dec tagged(target: unknown, name: string);
  extern fn buildName(prefix: string, count: int32): string;
  fn identity<T>(value: T): T;

  @sealed
  @tagged("fixture")
  model Decorated {
    value: string;
  }

  namespace Nested.Tools {
    op echo<T>(value: T): T;
    op clone(source: User) is Users.read;
    model Local {
      path: "nested";
      flags: #[true, false];
    }
  }
}

@@tagged(Fixture.Stress.Decorated, "augmented")
op health(): { ok: true, note: "fully closed 🚀" };
