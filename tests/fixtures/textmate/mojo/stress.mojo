"""Telemetry helpers used by a small sensor-processing demo.

The fixture keeps Unicode in harmless prose: café, naïve, λ, 漢字, 🚀, and 𝄞.

```mojo
fn tiny_example(value: Int) -> Int:
    return value + 1
```
"""

from collections import Dict, List
from math import sqrt
import sys
import time as clock

alias DEFAULT_WINDOW = 8
alias SAMPLE_BITS = 0b1111_1111
let MODULE_NAME = "telemetry"


trait Renderable:
    fn render(self) -> String:
        ...


trait Accumulator(CollectionElement):
    fn push(inout self, value: Float64):
        ...

    fn total(self) -> Float64:
        ...


@fieldwise_init
struct Reading(Renderable):
    var channel: String
    var value: Float64
    var valid: Bool

    fn render(self) -> String:
        return f"{self.channel}: {self.value:8.3f}"

    fn scaled(self, factor: Float64 = 1.0) -> Self:
        return Self(self.channel, self.value * factor, self.valid)


struct RunningStats(Accumulator):
    var count: Int
    var sum: Float64
    var sum_squares: Float64

    fn __init__(out self):
        self.count = 0
        self.sum = 0.0
        self.sum_squares = 0.0

    fn push(inout self, value: Float64):
        self.count += 1
        self.sum += value
        self.sum_squares += value ** 2

    fn total(self) -> Float64:
        return self.sum

    fn mean(self) -> Float64:
        if self.count == 0:
            return 0.0
        return self.sum / self.count

    fn variance(self) -> Float64:
        let average = self.mean()
        return max(0.0, self.sum_squares / self.count - average * average)


class Dashboard(Renderable):
    var title: String
    var rows: List[Reading]

    fn __init__(out self, title: String):
        self.title = title
        self.rows = List[Reading]()

    fn append(inout self, owned reading: Reading):
        self.rows.append(reading)

    fn render(self) -> String:
        return f"Dashboard({self.title!r}, rows={len(self.rows)})"


@always_inline
fn clamp[T: DType](value: SIMD[T, 1], low: SIMD[T, 1], high: SIMD[T, 1]) -> SIMD[T, 1]:
    return min(max(value, low), high)


fn decode_flags(bits: Int) -> Dict[String, Bool]:
    let masks = {
        "ready": 0x01,
        "warning": 0x02,
        "remote": 0o10,
    }
    var flags = Dict[String, Bool]()
    for name, mask in masks.items():
        flags[name] = (bits & mask) != 0
    return flags


fn parse_line(
    borrowed line: String,
    separator: String = ",",
    strict: Bool = False,
) raises -> Reading:
    let parts = line.split(separator)
    if len(parts) < 2:
        raise Error("missing measurement")
    try:
        let measured = Float64(parts[1].strip())
        return Reading(parts[0].strip(), measured, True)
    except:
        if strict:
            raise
        return Reading(parts[0].strip(), Float64("nan"), False)


fn normalize[width: Int = DEFAULT_WINDOW](
    values: List[Float64],
    inout stats: RunningStats,
) -> List[Float64]:
    comptime assert width > 0
    var cleaned = List[Float64]()
    for index in range(len(values)):
        let bounded = clamp(values[index], -1.0e3, +1_000.0)
        stats.push(bounded)
        cleaned.append(bounded)

    let center = stats.mean()
    let spread = sqrt(stats.variance()) or 1.0
    return [(item - center) / spread for item in cleaned]


fn select_channels(readings: List[Reading], wanted: List[String]) -> List[Reading]:
    let allowed = set(wanted)
    return [item for item in readings if item.valid and item.channel in allowed]


fn summarize(values: List[Float64]) -> tuple[Int, Float64, Float64]:
    if not values:
        return (0, 0.0, 0.0)
    let ordered = sorted(values)
    let midpoint = len(ordered) // 2
    return (len(values), ordered[midpoint], sum(values))


fn classify(value: Int) -> String:
    match value:
        case 0:
            return "idle"
        case 1 | 2:
            return "warming"
        case _ if value < 0:
            return "invalid"
        case _:
            return "active"


fn rolling_checksum(data: List[Int]) -> Int:
    var checksum = 0x811C9DC5
    for byte in data:
        checksum ^= byte & SAMPLE_BITS
        checksum *= 0x01000193
        checksum = (checksum << 5) | (checksum >> 27)
    return checksum


fn make_formatter(prefix: String) capturing:
    return lambda owned text, suffix="!": f"{prefix}: {text}{suffix}"


async fn fetch_snapshot(endpoint: String) raises -> String:
    let response = await request(endpoint)
    if response.status >= 400:
        raise Error(f"request failed: {response.status}")
    return await response.text()


async fn retry(endpoint: String, attempts: Int = 3) raises -> String:
    var remaining = attempts
    while remaining > 0:
        try:
            return await fetch_snapshot(endpoint)
        except Error as failure:
            remaining -= 1
            if remaining == 0:
                raise failure
            continue
        finally:
            clock.sleep(0.01)
    return "unreachable"


fn string_samples(name: String, value: Float64) -> List[String]:
    let escaped = "tab:\t newline:\n snowman:\u2603"
    let raw_path = r"^/sensors/(?P<name>[A-Za-z_]+)/(?:latest|\d{4})$"
    let bytes_header = b"MOJO\x00\x01"
    let raw_bytes = rb"packet\x20payload"
    let percent_style = "channel=%(name)-12s value=%08.2f"
    let brace_style = "channel={name!r:>12} value={value:.2f}"
    let interpolated = f"{name.upper()} => {value:+.2f} {{ok}}"
    return [escaped, raw_path, str(bytes_header), str(raw_bytes), percent_style, brace_style, interpolated]


fn multiline_samples(channel: String) -> tuple[String, String, String]:
    let note = """First line for café.
Second line carries 漢字 and an astral rocket 🚀.
The delimiters are balanced and this sentence closes the note.
"""
    let template = f'''channel = {channel!r}
status = {classify(2):>8}
literal braces = {{ready}}
'''
    let expression = r'''(?x)
^(?P<key>[A-Z_]+)\s*=\s*(?=\S)(?!disabled)(.+)$
'''
    return (note, template, expression)


fn inspect_scope(value: Int):
    global MODULE_NAME
    let label = `diagnostic-channel`
    assert value >= 0, "value must be nonnegative"
    # type: Dict[str, int]
    let metadata = {"value": value, "octets": value.bit_length()}
    # NOTE: λ is BMP; 𝄞 and 🛰️ exercise astral-safe comments.
    print(label, metadata.get("value", None))


fn main() raises:
    let source = ["temperature,21.5", "pressure,101.3", "broken,n/a"]
    var dashboard = Dashboard("Orbital café 🛰️")
    var stats = RunningStats()
    for line in source:
        let reading = parse_line(line)
        dashboard.append(reading)
        if reading.valid:
            stats.push(reading.value)

    let values = [row.value for row in dashboard.rows if row.valid]
    let report = {
        "summary": summarize(values),
        "flags": decode_flags(0b0000_1011),
        "checksum": rolling_checksum([1, 2, 3, 255]),
    }
    comptime if DEFAULT_WINDOW > 4:
        print("wide smoothing window")
    else:
        pass

    print(dashboard.render())
    print(report["summary"], normalize(values, stats))
    print(string_samples("temp", 21.5)[-1])
