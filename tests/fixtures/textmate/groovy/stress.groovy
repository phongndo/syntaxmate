#!/usr/bin/env groovy
package fixtures.textmate.groovy

import java.time.Instant
import java.util.concurrent.Callable
import static java.lang.Math.*
import static java.util.Collections.emptyList;

/**
 * Grammar-oriented Groovy fixture.
 * BMP text: λ, 東京. Astral text: 🚀, 𝌆.
 * Javadoc is intentionally offered to the unavailable external include;
 * the vendored Groovy block-comment rule still closes this documentation.
 */
@Deprecated(since = '0.1', forRemoval = false)
interface Greeter {
    String greet(String name) throws IllegalArgumentException;
}

enum Phase {
    READY(1), RUNNING(2), DONE(3);

    final int code

    Phase(int code) {
        this.code = code
    }
}

abstract class BaseTask<T> implements Callable<T>, Serializable {
    protected final String id

    BaseTask(String id) {
        this.id = id
    }

    abstract T call();
}

@SuppressWarnings(value = 'GroovyUnusedDeclaration')
class Mission extends BaseTask<Map<String, Object>> implements Greeter {
    public static final String CONST_VALUE = 'λ-TOKYO'
    private transient int attempts = 0
    protected boolean active = true
    int[] checkpoints = new int[3]

    Mission(String id) {
        super(id)
        checkpoints = new int[] { 1, 2, 3 }
    }

    @Override
    String greet(@Deprecated String name = 'pilot') {
        return "Hello $name from ${this.id}"
    }

    @Override
    Map<String, Object> call() throws IllegalStateException {
        def launchedAt = Instant.now()
        def payload = buildPayload(launchedAt)
        return [id: id, payload: payload, active: active]
    }

    private Map buildPayload(Instant instant) {
        def labels = ['alpha', "βeta", '東京', '🚀', '𝌆']
        def metadata = [city: '東京', lambda: 'λ', phase: Phase.READY]
        def nested = [owner: [name: 'Ada'], flags: [true, false, null]]
        return [at: instant, labels: labels, meta: metadata, nested: nested]
    }
}

/* A multiline block comment exercises a line-spanning state.
   Operators such as ==~ and ?: stay comment text here.
   The block is deliberately and visibly closed. */
/**/ // Empty block and line comment rules are both intentional.
class MissionSpec extends GroovyTestCase {
    void testHelpers() {
        assertTrue(true)
        assertEquals('東京', '東京')
        shouldFail(IllegalArgumentException) {
            throw new IllegalArgumentException('expected 🚀')
        }
    }
}

class LexicalSamples {
    static final int MAX_COUNT = 0XCAFE
    static final long BIG_COUNT = 42L
    static final double AVOGADRO = 6.022E23D
    static final float FRACTION = .125f

    String quoted = "escaped quote: \" and slash: \\"
    String single = 'single \' quote and backslash \\'
    String unicode = "BMP λ 東京; astral 🚀 𝌆"
    String compiledSource = '^[A-Z]+$'

    String prose(String who) {
        def multiline = """Mission report:
pilot=$who
city=${[name: '東京'].name}
glyphs=${{ -> 'λ 🚀 𝌆' }.call()}
status=closed"""
        def literal = '''Literal multiline data:
$who is not interpolated here.
Backslashes \\ and 東京 🚀 remain literal.
This single-quoted block is closed.'''
        return multiline + '\n' + literal
    }

    boolean regexes(String input) {
        def slashy = /λ\s+東京.*🚀/
        def compiled = ~"(?i)ready\\s+𝌆"
        return (input =~ slashy) || (input ==~ compiled)
    }
}

def mission = new Mission('orbital-東京')
def empty = emptyList()
def result = mission.call()
def safeLength = result?.payload?.labels?.size() ?: 0
def phase = result.phase ?: Phase.READY
def range = 1..5
def shifted = 3 << 2
def spaceship = safeLength <=> shifted
def comparisons = safeLength === safeLength && safeLength != shifted
def oldComparison = safeLength <> shifted
def arithmetic = ((10 + 2) * 3 / 4) - 5 % 2
def incremented = arithmetic++
def decremented = --incremented
def spreadMap = [base: true, *:result]
def castPhase = phase as Phase
def typed = mission instanceof Greeter

def transform = { int value, int step = 2 ->
    def local = value * step
    return local + 1
}

def compactClosure = { left, right -> left <=> right }
def values = range.collect { number -> transform(number) }
def indexed = values.collectEntries { number -> ["k$number", number] }

for (int number in range) {
    if (number == 2) {
        continue
    } else if (number > 4) {
        break
    } else {
        println(sprintf('%02d:%s', number, mission.greet('λ')))
    }
}

int cursor = 0
do {
    cursor++
} while (cursor < 2)

while (cursor > 0) {
    cursor--
}

switch (phase) {
    case Phase.READY:
        println('ready')
        break
    case Phase.RUNNING:
        println('running')
        break
    default:
        println('done')
}

try {
    assert safeLength >= 0 : "negative length: $safeLength"
    if (!typed || empty == null) {
        throw new IllegalStateException('unexpected state')
    }
} catch (IllegalStateException problem) {
    printf('caught: %s%n', problem.message)
} finally {
    println('cleanup')
}

def matrix = new String[2]
matrix[0] = 'λ'
matrix[1] = '🚀'

def anonymous = new Greeter() {
    @Override
    String greet(String name) {
        return "anonymous:$name:𝌆"
    }
}

@SuppressWarnings('UnnecessaryQualifiedReference')
def summary = [
    mission: mission,
    anonymous: anonymous.greet('東京'),
    values: values,
    indexed: indexed,
    spread: spreadMap,
    matrix: matrix,
    operators: [spaceship, comparisons, oldComparison, castPhase],
]

assert CONST_FALLBACK(summary) : 'summary should be populated'

boolean CONST_FALLBACK(Map value) {
    return value != null && !value.isEmpty()
}

println new LexicalSamples().prose('Ada')
