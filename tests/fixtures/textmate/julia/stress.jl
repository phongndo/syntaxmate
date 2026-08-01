#!/usr/bin/env julia
module TextMateStress
using Dates
import Base: show
export Mission, run_report
# Grammar-driven Julia fixture: café, 東京, λ, Ω, 🚀, and 𝌆.
const VERSION_TAG = v"1.4.0"
const DEFAULT_LIMIT = 4
const GLOBALS = (ARGS, ENV, VERSION, stdout, nothing, missing, true, false)
abstract type Payload end
struct Reading{T<:Real} <: Payload
    name::Symbol
    value::T
end
mutable struct Mission{T}
    id::Int
    title::String
    readings::Vector{Reading{T}}
end
primitive type TinyCode 16 end
macro tagged(ex)
    return :(println("tagged: ", $(esc(ex))))
end
identity_typed(x::T)::T where {T} = x
scale(x, factor=2) = x * factor
function show(io::IO, mission::Mission)
    print(io, "Mission($(mission.id), $(repr(mission.title)))")
end
"""
Build a deterministic mission containing Unicode labels.

The source includes BMP text `東京`, `λ`, and astral `🚀`, `𝌆`.
"""
function build_mission(id::Integer; limit::Integer=DEFAULT_LIMIT)
    readings = Reading{Float64}[
        Reading(:alpha, 1.25),
        Reading(:βeta, 0x2a),
        Reading(:orbit, 6.022e23),
    ]
    selected = readings[begin:min(end, limit)]
    return Mission(Int(id), "東京 λ 🚀 𝌆", selected)
end
#= Outer stateful comment begins.
   TODO: nested comments should remain nested.
   #= Inner comment contains FIXME and [brackets] "quotes". =#
   CHANGED: both levels close before executable code resumes.
=#

function summarize(mission::Mission)
    totals = Dict{Symbol,Float64}()
    for reading in mission.readings
        totals[reading.name] = get(totals, reading.name, 0.0) + reading.value
    end

    rows = [
        (name=name, value=value, scaled=scale(value, 3))
        for (name, value) in totals
        if isfinite(value)
    ]
    return rows
end

function control_flow(limit)
    total = 0
    index = 0
    while index < limit
        index += 1
        if isodd(index)
            total += index
        elseif index == 2
            continue
        else
            total -= 1
        end
    end

    result = try
        total > 0 ? total : throw(DomainError(total))
    catch error
        @warn "recovering from $error"
        zero(total)
    finally
        @debug "control flow complete"
    end
    return result
end

function numeric_and_operator_cases(x)
    integers = (1_000, 0xff, 0o755, 0b1010, -42)
    floats = (.125, 3.0f2, 2.5e-4, Inf, NaN, π, ℯ, 2im)
    shifted = (0xff << 2) >>> 1
    compared = x isa Real && 0 <= x < 100 || x === nothing
    mapped = integers .+ 1 |> collect
    range = firstindex(mapped):2:lastindex(mapped)
    relation = x ∈ mapped ? :member : :outsider
    pair = relation => (shifted, compared, floats)
    return pair, range, mapped', mapped .^ 2
end

function string_cases(name)
    ordinary = "hello $name; expression=$(uppercase(name)); quote=\"; λ 🚀"
    character = '\u03bb'
    escaped = "tab=\t hex=\x41 scalar=\U0001F680"
    multiline = """first line for $name
second line computes $(join(["東京", "λ", "🚀", "𝌆"], ", "))
third line keeps \\ escapes and closes below"""
    raw_one = raw"C:\fixtures\julia\$name"
    raw_many = raw"""raw multiline
no interpolation for $name and path C:\tmp\𝌆
raw delimiter closes here"""
    symbol_one = var"mission status"
    symbol_many = var"""mission
display name"""
    custom = html"<b>café</b>"
    custom_many = markdown"""# Mission
Text for 東京 and 🚀.
"""
    command = `printf '%s' $name`
    command_many = ```printf '%s\n' café
printf '%s\n' 🚀```
    regex_one = r"^(?<kind>alpha|βeta)-\d+$"im
    regex_many = r"""(?x)
        ^ (東京|café) \s+ [0-9]+ $
    """ms
    return (; ordinary, character, escaped, multiline, raw_one, raw_many,
            symbol_one, symbol_many, custom, custom_many, command,
            command_many, regex_one, regex_many)
end

@doc doc"""A macro-style doc string.
It interpolates the fixture name: $(nameof(TextMateStress)).
""" ->
function documented(value)
    value
end

cpp_source = cxx"""
#include <array>
#define TWICE(v) ((v) + (v))
template <typename T>
T clamp_value(T value, T high) {
    std::array<T, 2> bounds{T{0}, high};
    return value < bounds[0] ? bounds[0] : TWICE(value);
}
auto unicode_note = u8"東京 λ 🚀 𝌆";
"""

python_source = py"""
from dataclasses import dataclass

@dataclass
class Orbit:
    name: str

def render(items):
    return [f"{item.name}:{index}" for index, item in enumerate(items)]

orbit = Orbit("東京 🚀")
"""

javascript_source = js"""
const glyphs = ["λ", "東京", "🚀", "𝌆"];
export function render(limit) {
  return glyphs.slice(0, limit).map((glyph, index) => ({ glyph, index }));
}
"""

r_source = R"""
readings <- data.frame(name = c("alpha", "βeta"), value = c(1.0, 2.5))
normalize <- function(xs) {
  (xs - mean(xs)) / sd(xs)
}
readings$scaled <- normalize(readings$value)
message("東京 λ 🚀 𝌆")
"""

sql_source = sql"""
WITH ranked AS (
  SELECT mission_id, title, score,
         ROW_NUMBER() OVER (PARTITION BY mission_id ORDER BY score DESC) AS rank
  FROM mission_readings
  WHERE score >= 0 AND title <> 'cancelled'
)
SELECT mission_id, title, score
FROM ranked
WHERE rank <= 4
ORDER BY mission_id, rank;
"""

function collection_cases(mission)
    lookup = Dict{Symbol,Tuple{Float64,String}}(
        :alpha => (1.0, "first"),
        :βeta => (2.0, "second"),
    )
    pairs = [(reading.name, reading.value) for reading in mission.readings]
    flattened = mapreduce(last, +, pairs; init=0.0)
    callback = (x, y) -> x + y
    composed = string ∘ identity_typed
    return lookup, flattened, callback(2, 3), composed(mission.id)
end

function run_report(; limit=DEFAULT_LIMIT)
    mission = build_mission(7; limit)
    @tagged mission.title
    report = let rows = summarize(mission)
        join(("$(row.name)=$(row.value)" for row in rows), "; ")
    end
    open(`echo $report`, "r") do io
        read(io, String)
    end
    return (; mission, report, code=control_flow(limit),
            strings=string_cases(mission.title), collections=collection_cases(mission),
            embedded=(cpp_source, python_source, javascript_source, r_source, sql_source))
end

if abspath(PROGRAM_FILE) == @__FILE__
    println(run_report())
end

end # module TextMateStress
