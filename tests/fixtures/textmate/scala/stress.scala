#!/usr/bin/env scala
//> using scala 3.6.4
//> using dep "org.typelevel::cats-core:2.12.0"
package fixtures.textmate.scala
import java.time.{Instant as Moment, *}
import scala.collection.mutable.{Map => MutableMap, _}
import scala.concurrent.{ExecutionContext, Future}
import scala.deriving.Mirror
import scala.quoted.*
import Ordering.Implicits.given

export Helpers.{unicode as glyphs, given}
/** Grammar stress fixture for [[Stress]], [[scala.collection.Seq]].
  *
  * It contains 東京, λ, 🚀, and 𝌆 in reviewable source text.
  * @tparam A a value type
  * @param seed initial value
  * @return a transformed value
  * @note Lexical states are deliberately multiline and all are closed.
  * @since 1.0
  */
@deprecated("fixture-only API", since = "0.1")
@SerialVersionUID(1L)
sealed abstract class Base[-A, +B <: Product](protected val seed: A)
    extends Serializable
    with Product:
  protected[scala] def transform(value: A): B
end Base
trait Codec[A]:
  def encode(value: A): String
  def decode(text: String): Either[String, A]
trait LegacyBounds[A <% Ordered[A], B >: Null <: AnyRef]:
  def compare(left: A, right: A)(implicit tag: reflect.ClassTag[B]): Int
case class Record[+A](id: Long, value: A, tags: List[String] = Nil)
    derives CanEqual
case object EmptyRecord
enum Color(val rgb: Int) derives CanEqual:
  case Red extends Color(0xff0000)
  case Green extends Color(0x00ff00)
  case Blue extends Color(0x0000ff)
  case Custom(override val rgb: Int)
end Color
opaque type UserId = Long
object UserId:
  def apply(value: Long): UserId = value
  extension (id: UserId)
    infix def max(other: UserId): UserId = math.max(id, other)
    def show: String = s"user-$id"
type Element[X] = X match
  case String      => Char
  case Array[t]    => t
  case Iterable[t] => t
  case _           => X
type LegacyExistential = List[T] forSome { type T <: Product }
type Structural = { def close(): Unit; val name: String }
given orderingRecord[A: Ordering as elementOrdering]: Ordering[Record[A]] with
  def compare(x: Record[A], y: Record[A]): Int =
    val byId = x.id.compare(y.id)
    if byId != 0 then byId else elementOrdering.compare(x.value, y.value)
given ExecutionContext = ExecutionContext.global
given Codec[Int] with
  def encode(value: Int): String = value.toString
  def decode(text: String): Either[String, Int] = text.toIntOption.toRight("NaN")
extension [A](values: List[A])
  def secondOption: Option[A] = values.drop(1).headOption
  infix def interleave(other: List[A]): List[A] =
    values.zipAll(other, null.asInstanceOf[A], null.asInstanceOf[A]).flatMap(_.toList)
end extension
open class Parent(val label: String):
  protected def prefix = "parent"
final class Child private[scala] (override val label: String)
    extends Parent(label):
  @volatile private var ticks = 0L
  @transient lazy val created: Moment = Moment.now()
  override protected def prefix: String = "child"
  synchronized { ticks += 1 }
object Helpers:
  val unicode: String = "λ 東京 🚀 𝌆"
  given Ordering[Color] = Ordering.by(_.rgb)
package object legacy:
  type Name = String
  implicit class RichName(private val self: Name) extends AnyVal:
    def shout: String = self.toUpperCase
object Literals:
  val truth = true
  val lie = false
  val nothing: String = null
  val decimal = 1_234_567
  val signed = -42L
  val hexadecimal = 0xCAFE_BABEL
  val binary = 0b1010_0110L
  val doubles = List(1.0, 6.022e23, 2.5E-4D, .125f, 9F)
  val escaped: String = "quote=\" slash=\\ tab=\t λ=\u03bb octal=\141"
  val chars = List('a', 'λ', '\n', '\'', '\\', '\u6771', '\141')
  val oldSymbol = 'legacySymbol
  val singleton: Literals.type = this
  val `strange value` = 7
  def `plus-ish_++`(n: Int): Int = n + 1
object Strings:
  val name = "東京"
  val rocket = "🚀"
  val plain = "plain string"
  val interpolated = s"Hello $name, ${rocket.reverse.reverse}; cost=$$5"
  val formatted = f"pi=${math.Pi}%1.3f count=${2 + 3}%02d"
  val rawSingle = raw"C:\tmp\東京\n $$ ${name}"
  val custom = StringContext("select ", "").s(name)
  val triplePlain = """A multiline plain string with "quotes", \\, λ, 東京, 🚀, and 𝌆.
The closing delimiter remains on the next line."""
  val tripleS = s"""A multiline interpolation:
name=$name expression=${List("λ", "東京", "🚀", "𝌆").mkString(" | ")}
literal dollars=$$ and slash=\\
done"""
  val tripleRaw = raw"""Raw multiline data:
C:\fixtures\scala\stress; interpolated=$name and dollars=$$
closed here"""
  val tripleF = f"""Formatted multiline:
pi=${math.Pi}%1.4f; hex=${255}%02x
done"""
object Comments:
  /* outer block starts
     /* nested level one
        /* nested level two: λ 東京 🚀 𝌆 */
        level one resumes
     */
     outer block ends
   */
  /** Documentation can nest a /* regular block */ and continue.
    * @param value documented value
    * @return [[Record]] data
    * @todo retain nested-comment coverage
    */
  def documented(value: Int): Record[Int] = Record(0L, value)
  /**/ // empty block, then a line comment
object Patterns:
  def classify(value: Any): String = value match
    case null                              => "null"
    case n: Int if n >= 0 && n <= 9       => "digit"
    case s @ ("λ" | "東京")               => s"word:$s"
    case Record(id, value, head :: tail)  => s"$id:$value:${head +: tail}"
    case (first, second, rest @ _*)       => s"$first:$second:$rest"
    case _: Product                       => "product"
    case _                                => "other"
  def collect(input: List[Option[Int]]): List[(Int, Int)] =
    for
      Some(n) <- input
      doubled = n * 2
      if doubled % 3 != 0
      index <- 0 until n
    yield (index, doubled)
  def oldFor(input: List[Int]) = for { n <- input; if n > 0
  } yield n * n
object Control:
  def loops(limit: Int): Int =
    var total = 0
    var index = 0
    while index < limit do
      total += index
      index += 1
    end while
    if total > 100 then
      return total
    else
      total + 1
    end if
  def guarded(body: => Int): Int =
    try body
    catch
      case error: ArithmeticException => throw IllegalArgumentException(error)
    finally println("finished")
    end try
object Operators:
  extension (left: Int)
    infix def <+>(right: Int): Int = left + right
  val arithmetic = (1 + 2) * 3 / 4 - 5 % 2
  val logic = !false && true || false
  val comparison = 1 <= 2 && 2 != 3 && 3 >= 3
  val arrows = List(1 -> "one", 2 → "two")
  val symbolic = 1 <+> 2
object XmlScala2:
  val id = 7
  val node =
    <ui:panel id="main" data-city='東京'>
      <title>{Strings.name} &amp; λ</title>
      <item enabled="true">{id + 1}</item>
      <empty />
    </ui:panel>
object Metaprogramming:
  inline def choose(inline condition: Boolean): Int =
    inline if condition then 1 else 0
  def quoted(using Quotes): Expr[Int] = '{ 40 + 2 }
  def quotedType(using Quotes): Type[List[Int]] = '[List[Int]]
  inline def spliceValue: Int = ${ spliceImpl }
  def spliceImpl(using Quotes): Expr[Int] = '{ 42 }
object Advanced:
  transparent inline def identity[A](inline value: A): A = value
  inline val constant: Int = 42
  def contextual[A: Ordering](values: List[A])(using codec: Codec[A]): String =
    values.sorted.map(codec.encode).mkString(",")
  def evidence[A, B](value: A)(implicit ev: A =:= B): B = ev(value)
  def subtype[A, B](value: A)(using ev: A <:< B): B = ev(value)
  def byName(thunk: => Int, repeated: String*): (Int, Seq[String]) =
    thunk -> repeated
  @native def nativeHash(value: Long): Int
object Stress:
  @main def run(seed: Int): Unit =
    val ids = List(UserId(seed.toLong), UserId(99L))
    val result = ids.head max ids.last
    val future = Future(Advanced.contextual(List(3, 1, 2)))
    println(s"${result.show}: $future; ${Helpers.unicode}")
end Stress
