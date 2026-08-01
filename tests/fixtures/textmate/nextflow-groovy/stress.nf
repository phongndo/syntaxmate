#!/usr/bin/env nextflow
/*
 * Standalone exercise for the Groovy subset embedded by Nextflow.
 * BMP text: λ, β, 東京. Astral text: 🚀, 🧬, 𝌆.
 * This comment deliberately spans lines and closes here.
 */

def launched = true
boolean dryRun = false
byte tiny = 7
char initial = 'N'
short port = 22
int retries = 3
long bases = 42000000000L
float ratio = .625f
double score = 6.022E23D
def hexMask = 0XCAFE
def unsignedHint = 12UL
def absent = null

String city = '東京'
String escaped = 'single quote: \' and slash: \\'
String greeting = "Hello $city, λ aboard 🚀"
String nestedGreeting = "sample=${[id: 'S1', meta: [group: 'β']].meta.group}"
String expression = "answer=${{ int left, int right -> left + right }(20, 22)}"

def literalBlock = '''Literal payload:
$city is not interpolated.
Backslash \\ and glyphs 東京 🚀 𝌆 stay literal.
The triple-single string closes now.'''

def reportBlock = """Mission report:
city=$city
status=${launched ? 'ready' : 'waiting'}
nested=${[owner: [name: 'Ada']].owner.name}
glyphs=${{ -> 'λ 🧬 𝌆' }.call()}
The triple-double string closes now."""

def readPattern = /reads_[12]\.f(ast)?q\.gz/
def pathPattern = /data\/[^/]+\/sample-[A-Z]+/
def compiledWord = ~"(?i)ready\\s+${city}"
def foundRead = 'reads_1.fastq.gz' =~ readPattern
def wholeRead = 'reads_2.fq.gz' ==~ readPattern

List<String> labels = ['alpha', "βeta", city, '🚀', '𝌆']
ArrayList<Integer> depths = new ArrayList<Integer>([10, 20, 30])
Map<String, Object> sample = [
    id: 'SAMPLE_01',
    active: true,
    stage: 'align',
    metrics: [reads: 1200, mapped: 1175],
    tags: labels,
    optional: null,
]
def emptyMap = [:]
def matrix = [[1, 2], [3, 4], [5, 6]]
def mixed = [name: '東京', numbers: [1, 2.5, -3], enabled: false]

def now = new Date()
def builder = new StringBuilder('prefix')
def uri = new URI('https://example.test/data')
def ordered = new LinkedHashMap<String, Integer>()
ordered.put('first', 1)
ordered.put('second', 2)
builder.append(':').append(sample.id)

def increment = { int value -> value + 1 }
def multiply = { int value, int factor = 2 ->
    def product = value * factor
    return product
}
def decorate = { String text = 'λ', String suffix = '🚀' ->
    "$text::$suffix"
}
def compare = { left, right -> left <=> right }
def noArgs = { -> [ok: true, city: city] }

def doubled = depths.collect { int depth -> multiply(depth) }
def indexed = doubled.collectEntries { int depth ->
    ["depth_$depth", increment(depth)]
}
def selected = labels.findAll { String label -> label.size() >= 2 }
def rendered = selected.collect { label -> decorate(label, '🧬') }
def total = depths.inject(0) { int sum, int depth -> sum + depth }

def safeId = sample?.metadata?.identifier ?: sample?.id ?: 'unknown'
def safeMissing = absent?.child?.name ?: 'fallback'
def range = 1..5
def descending = 5..1
def membership = 3 in range
def castRetries = retries as long
def isMap = sample instanceof Map
def shifted = retries << 2
def ordering = total <=> shifted
def legacyNotEqual = total <> shifted
def arithmetic = ((10 + 2) * 3 / 4) - 5 % 2
def logic = launched && !dryRun || absent == null
def ternary = membership ? 'inside' : 'outside'
def elvis = sample.optional ?: 'not-set'

retries++
--retries
builder << ':streamed'

if (logic && isMap) {
    builder.append(':valid')
} else {
    builder.append(':invalid')
}

for (int index in range) {
    if (index == 2) {
        builder.append(':skip')
    } else {
        builder.append(":$index")
    }
}

int cursor = 0
while (cursor < 2) {
    cursor++
    builder.append(":w$cursor")
}

try {
    assert total >= 0 : "negative total: $total"
    if (!wholeRead) {
        throw new IllegalArgumentException('read name did not match')
    }
} catch (IllegalArgumentException problem) {
    builder.append(":caught-${problem.message}")
}

def taskConfig = [
    process: [
        cpus: 4,
        memory: '8 GB',
        time: '2h',
    ],
    executor: [name: 'local', queueSize: 16],
    tracing: [enabled: true, file: 'trace.txt'],
]

def mergeMaps = { Map base, Map extra = [:] ->
    def merged = [:]
    base.each { key, value -> merged[key] = value }
    extra.each { key, value -> merged[key] = value }
    return merged
}

def mergedConfig = mergeMaps(
    taskConfig,
    [unicode: [bmp: 'λ 東京', astral: '🚀 𝌆']]
)

def summarize = { Map record ->
    def metric = record.metrics?.mapped ?: 0
    def state = record.active ? 'active' : 'disabled'
    return [id: record.id, metric: metric, state: state]
}

def summary = summarize(sample)
def table = matrix.collect { row ->
    def left = row[0]
    def right = row[1]
    [left: left, right: right, sum: left + right]
}

def formatted = table.collect { Map row ->
    sprintf('%d+%d=%d', row.left, row.right, row.sum)
}.join(', ')

def choosePath = { String root, String name = 'result.txt' ->
    def cleanRoot = root?.trim() ?: '.'
    return "$cleanRoot/$name"
}

def outputPath = choosePath('/tmp/東京', 'report-🚀.txt')
def constants = [READY_STATE, MAX_RETRIES, PIPELINE_2]
def flags = [true, false, null]

assert dryRun == false : 'dry-run flag changed'
assert foundRead : 'slashy search should find a read'
assert wholeRead : 'slashy whole match should succeed'
assert compiledWord.matcher('READY 東京').find() : 'compiled regex failed'
assert compare(1, 2) < 0 : 'comparison closure failed'
assert noArgs().ok : 'zero-argument closure failed'
assert safeId == 'SAMPLE_01' : greeting
assert safeMissing == 'fallback' : reportBlock
assert castRetries >= 0 && ordering != null : expression
assert legacyNotEqual || total == shifted : 'comparison state'
assert mergedConfig.unicode.astral == '🚀 𝌆' : literalBlock
assert outputPath.endsWith('.txt') : outputPath

println(greeting)
println(nestedGreeting)
println(rendered)
println(indexed)
println(formatted)
println(builder.toString())
