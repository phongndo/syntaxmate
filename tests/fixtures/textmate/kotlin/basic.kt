package fixtures.kotlin.basic

/* Basic Kotlin fixture:
 * café λ 🚀 𝌆
 */
data class Pilot(val name: String, val active: Boolean = true)

fun describe(pilot: Pilot?): String {
    val status = when {
        pilot == null -> "missing"
        pilot.active -> "ready"
        else -> "resting"
    }
    return "${pilot?.name ?: "Nobody"}: $status"
}

fun main() {
    val banner = """
        |Mission café 🚀
        |Signal: λ 𝌆
    """.trimMargin()
    val pilots = listOf(Pilot("Ada"), Pilot("Lin", active = false))
    pilots.filter { it.active }.forEach { pilot ->
        println("$banner\n${describe(pilot)}")
    }
}
