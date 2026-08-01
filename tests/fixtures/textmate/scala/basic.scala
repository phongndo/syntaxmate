package fixtures.scala

import scala.collection.immutable.{List, Map as Dict}

/** A tiny [[Basic]] fixture.
  * @param name display name
  * @return a Unicode greeting
  */
@deprecated("fixture only", "1.0")
sealed trait Greeting derives CanEqual
case class Hello(name: String, count: Int = 1) extends Greeting
case object Goodbye extends Greeting

object Basic:
  val city = "東京"
  val glyphs = "λ 🚀 𝌆"
  val message = s"Hello, ${city}! $$count=${Hello(city).count}"
  val poem = """first line
               |second line""".stripMargin
  def render(greeting: Greeting): String = greeting match
    case Hello(name, n) if n > 0 => s"$name × $n"
    case Goodbye                 => raw"bye\n"
end Basic
