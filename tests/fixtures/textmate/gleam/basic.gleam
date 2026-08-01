// Gleam basics: 東京, λ, 🚀, and 𝌆
import gleam/io as console
pub type Mission { Mission(name:String, active: Bool) }
opaque type Secret { Secret(code: Int) }
pub const launch_code = 0xCA_FE
const masks = #(0b1010_0110, 0o755, 1_000, 6.02e-23)

pub fn describe(mission: Mission, _trace: Bool) -> String {
  let Mission(name: label, ..) = mission
  let decorated = "東京 \"λ\" 🚀 𝌆" <> label
  assert decorated != "" && 10 >= 2 || 1 < 0
  case mission {
    Mission(active: True, ..) if 3.5 >=. 2.0 -> decorated
    Mission(active: False, ..) -> panic as "inactive"
  }
}

fn calculate(x: Int, y: Float) {
  let ints = x + 2 - 3 * 4 / 5 % 2
  let floats = y +. 1.0 -. 2.0 *. 3.0 /. 4.0
  ints |> echo
}

fn receive_message() {
  use value <- result.try(todo)
  if value == 42 { console.print("ok") } else { console.print("no") }
}
