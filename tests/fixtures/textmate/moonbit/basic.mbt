/// A tiny Unicode-aware greeting.
pub struct Greeter {
  prefix : String
}

pub fn Greeter::greet(self : Greeter, name : String) -> String {
  let decorated = "\{self.prefix}, \{name}! 🚀"
  decorated
}

fn main {
  let greeter = Greeter::{ prefix: "Bonjour café" }
  let names = ["Ada", "東京", "λ"]
  for name in names {
    println(greeter.greet(name))
  }
  let answer = 0x2A
  if answer >= 42 && true {
    println("meaning = \{answer}")
  }
}
