@file:JvmName("TelemetryDemo")
@file:Suppress("unused", "MemberVisibilityCanBePrivate")
package fixtures.kotlin.stress

import java.time.Instant
import java.util.Locale as JavaLocale
import kotlin.math.abs
import kotlin.properties.Delegates
// The model intentionally uses both JVM types and Kotlin extensions.
/**
 * A compact telemetry model used to exercise the Kotlin grammar.
 * @param T payload carried by an event
 * @property id stable event identifier
 * @author café-team λ
 */
sealed interface Event<out T> {
    val id: String
    val payload: T
    fun describe(): String
}

@Target(AnnotationTarget.CLASS, AnnotationTarget.FUNCTION)
@Retention(AnnotationRetention.RUNTIME)
annotation class Audited(val owner: String, val enabled: Boolean = true)
typealias Attributes = MutableMap<String, List<String>>
typealias Handler<T> = suspend (Event<T>) -> Result<Unit>

@Audited(owner = "ops")
data class Metric<T : Number>(
    override val id: String,
    override val payload: T,
    val attributes: Attributes = mutableMapOf(),
) : Event<T> {
    override fun describe(): String = "metric[$id]=${payload.toDouble()}"
    operator fun plus(other: Metric<T>): Double =
        payload.toDouble() + other.payload.toDouble()
}

data class Message(
    override val id: String,
    override val payload: String,
) : Event<String> {
    override fun describe() = payload.ifEmpty { "<empty>" }
}

sealed class Delivery {
    data object Pending : Delivery()
    data class Sent(val at: Instant) : Delivery()
    data class Failed(val reason: String, val retryable: Boolean) : Delivery()
}
enum class Priority(val weight: Int) {
    LOW(1), NORMAL(5), HIGH(10);
    fun urgent(): Boolean = this == HIGH
}

@JvmInline
value class EventId(val raw: String) {
    init {
        require(raw.isNotBlank()) { "event id must not be blank" }
    }
    override fun toString(): String = raw
}
fun interface Encoder<in T> {
    fun encode(value: T): ByteArray
}

open class Repository<T : Event<*>>(private val capacity: Int = 128) {
    private val events = mutableListOf<T>()
    var accepted: Int by Delegates.observable(0) { _, old, new ->
        check(new >= old)
    }
    open fun add(event: T): Boolean {
        if (events.size >= capacity) return false
        events += event
        accepted++
        return true
    }
    fun snapshot(): List<T> = events.toList()
    companion object Factory {
        const val DEFAULT_CAPACITY = 128
        fun <T : Event<*>> create(): Repository<T> = Repository(DEFAULT_CAPACITY)
    }
}

class IndexedRepository<T>(capacity: Int) : Repository<T>(capacity)
    where T : Event<*>, T : Comparable<T> {
    private val index by lazy { sortedMapOf<String, T>() }
    override fun add(event: T): Boolean =
        super.add(event).also { stored -> if (stored) index[event.id] = event }
}

object EventRegistry {
    private val handlers: MutableMap<String, (Event<*>) -> Unit> = mutableMapOf()
    fun register(key: String, handler: (Event<*>) -> Unit) {
        handlers[key] = handler
    }
    fun dispatch(event: Event<*>) = handlers[event.id]?.invoke(event)
}
val Event<*>.summary: String
    get() = "${this::class.simpleName}: ${describe()}"
fun String.normalized(locale: JavaLocale = JavaLocale.ROOT): String =
    trim().lowercase(locale).replace(' ', '-')
infix fun Event<*>.taggedAs(tag: String): Pair<Event<*>, String> = this to tag
inline fun <reified T : Event<*>> Iterable<Event<*>>.firstTyped(): T? {
    for (event in this) {
        if (event is T) return event
    }
    return null
}
tailrec fun retryDelay(attempt: Int, current: Long = 1L): Long {
    return if (attempt <= 0) current else retryDelay(attempt - 1, current * 2)
}
/* Delivery converts handler outcomes into a sealed result. */
@Audited("runtime")
suspend fun <T> deliver(event: Event<T>, handler: Handler<T>): Delivery {
    return try {
        handler(event).getOrThrow()
        Delivery.Sent(Instant.now())
    } catch (cancelled: InterruptedException) {
        throw cancelled
    } catch (failure: Exception) {
        Delivery.Failed(failure.message ?: "unknown", retryable = true)
    } finally {
        println("delivery attempted for ${event.id}")
    }
}

private fun classify(score: Int): Priority = when {
    score < 0 -> throw IllegalArgumentException("negative score")
    score in 0..3 -> Priority.LOW
    score in 4..7 -> Priority.NORMAL
    else -> Priority.HIGH
}

fun collectReadings(limit: Int): List<Int> {
    val values = mutableListOf<Int>()
    var cursor = 0
    while (cursor < limit) {
        cursor++
        if (cursor % 2 == 0) continue
        values += cursor
        if (values.size >= 5) break
    }
    do {
        cursor--
    } while (cursor > limit)
    return values
}

private fun literals(): Map<String, Any?> {
    val binary = 0b1010_0110
    val hex = 0xCAFE_BABEu
    val scientific = 6.022e23
    val ratio = 12.5F
    val marker = '\u03bb'
    val escaped = "quote=\" slash=\\ newline=\n"
    val empty = ""
    val launch = "café 東京 🚀 $marker"
    return mapOf("binary" to binary, "hex" to hex, "number" to scientific,
        "ratio" to ratio, "escaped" to escaped, "empty" to empty, "launch" to launch)
}

private fun report(events: List<Event<*>>): String {
    val header = """
        |Telemetry report 🚀
        |count=${events.size}
        |first=${events.firstOrNull()?.describe() ?: "none"}
    """.trimMargin()
    return events.joinToString(prefix = "$header\n", separator = "\n") { event ->
        event.summary
    }
}

@Suppress("MagicNumber")
fun main() {
    val repository = Repository.create<Event<*>>()
    val samples = listOf(
        Metric("cpu", 73.5, mutableMapOf("host" to listOf("alpha", "βeta"))),
        Message("status", "Ready to launch 🚀"),
    )
    samples.filter { it.id.isNotEmpty() }.forEach(repository::add)
    EventRegistry.register("status") { event -> println(event.summary) }
    repository.snapshot().forEach(EventRegistry::dispatch)

    val (event, tag) = samples.first() taggedAs "demo"
    val priority = classify(abs(7))
    println("${event.describe()} #$tag priority=$priority")
    println(report(repository.snapshot()))
    check(literals()["hex"] != null && retryDelay(3) == 8L)
}
