// TypeSpec basics: café λ 東京 🚀 𝌆
import "@typespec/http";
using TypeSpec.Http;

namespace Fixture.Basic;

/** A visible payload.
 * @template T payload type
 * @prop note localized note
 */
@doc("Unicode café λ 東京 🚀 𝌆")
model Envelope<T extends string = string> {
  id: int64;
  note?: "ready";
  payload: T;
  details: """
    multiline ${T}
    closes here 🚀
  """;
}

enum State { ready, `in-flight`: "flying" }
union Result { ok: Envelope<string>, failed: { code: int32 } }
op read(id: string, options?: #{ trace: true }): Result;
