#!/usr/bin/env luau
--- A hand-reviewable typed Luau telemetry module.
--- Unicode samples: café, λ, 東京, 🚀, 𝌆.
--- @class Telemetry
--- @within Fixtures
-- Ordinary comments may mention if, function, ::, and @native without acting.
--[=[
Block comment with balanced-looking source:
```luau
local phantom: string = `not executed {42}`
```
The shorter [[ delimiter ]] does not close this comment.
]=]
export type Identifier = string
export type Phase = "idle" | "collecting" | "complete" | "failed"
export type Pair<T, U> = { first: T, second: U }
export type Dictionary<T> = { [string]: T }
export type Page<T = string> = { items: {T}, cursor: string? }
export type Result<T, E = string> = { ok: true, value: T } | { ok: false, error: E }
export type Reducer<T> = (current: T, sample: number) -> T
export type Callback<T...> = (T...) -> ()
export type Accessors = {
    read name: string,
    write pending: boolean,
    [number]: string,
}
export type Snapshot = {
    id: Identifier,
    phase: Phase,
    count: number,
    tags: {string},
    values: Dictionary<number>,
}
type Maybe<T> = T | nil
type Scalar = number | boolean | string
type Combined = Snapshot & { updatedAt: number }
type Factory = <T>(seed: T) -> Pair<T, T>
type Packed<T...> = (T...) -> T...
type RuntimeClock = typeof(os.clock)
type Nothing = never
type Anything = unknown
type function OptionalOf(inner)
    return types.unionof(inner, types.singleton(nil))
end
export type function Duplicate(inner)
    return types.newtable({ first = inner, second = inner })
end
local DEFAULT_LIMIT: number = 25
local DEFAULT_PHASE: Phase = "idle"
local enabled: boolean, lastError: string? = true, nil
local decimal: number = 12_345.625
local leading: number = .75
local exponent: number = 6.022e23
local tiny: number = 1.5E-10
local hexadecimal: number = 0xDEAD_BEEF
local binary: number = 0b1010_0110
local total: number = decimal + leading * 2 - tiny
local quotient = total / 4
local floored = total // 4
local remainder = floored % 7
local powered = remainder ^ 2
local single = 'single quoted: \'café\''
local double = "double quoted: \"東京\""
local escapes = "line\n tab\t slash\\ byte\065 hex\x42 unicode\u{1F680}"
local unicode = "λ / 𝌆"
local longPlain = [[
Long strings keep "quotes", -- markers, and {braces} literal.
They also keep café, 東京, and 🚀 on a second line.
]]
local longEqual = [==[
An equals-delimited string contains [[short]] and [=[medium]=] delimiters.
Everything remains literal until the matching close.
]==]
local interpolated = `phase={DEFAULT_PHASE}, total={total}, icon=🚀`
local nestedInterpolation = `label={string.upper(`café-{unicode}`)}`
local counters: Dictionary<number> = {
    accepted = 0,
    rejected = 0,
    ["東京"] = 1,
}
local samples: {number} = { decimal, leading, exponent; tiny }
local metadata = {
    title = "Telemetry",
    active = enabled,
    nested = { lambda = "λ", symbol = "𝌆" },
    [1 + 1] = "computed key",
}
@native
local function clamp(value: number, low: number, high: number): number
    if value < low then
        return low
    elseif value > high then
        return high
    else
        return value
    end
end

local function fold<T>(values: {number}, initial: T, reducer: Reducer<T>): T
    local current: T = initial
    for index: number, value: number in ipairs(values) do
        if index % 2 == 0 then
            current = reducer(current, value)
        else
            current = reducer(current, -value)
        end
    end
    return current
end
--- Format one snapshot for logs.
--- @param snapshot snapshot to render
--- @param prefix optional display prefix
local function formatSnapshot(snapshot: Snapshot, prefix: string?): string
    local heading = prefix or "snapshot"
    return `{heading}:{snapshot.id}:{snapshot.phase}:{snapshot.count}`
end

local function pack<T...>(...: T...): T...
    return ...
end

local function makePair<T>(seed: T): Pair<T, T>
    return { first = seed, second = seed }
end

local Telemetry = {}
Telemetry.__index = Telemetry

function Telemetry.new(id: Identifier): Snapshot
    local cleanId = if #id > 0 then id else "anonymous"
    return {
        id = cleanId,
        phase = DEFAULT_PHASE,
        count = 0,
        tags = {},
        values = {},
    }
end

function Telemetry.increment(snapshot: Snapshot, key: string, amount: number?): number
    local delta = amount or 1
    local previous = snapshot.values[key] or 0
    local nextValue = previous + delta
    snapshot.values[key] = nextValue
    snapshot.count += 1
    return nextValue
end

function Telemetry:describe(prefix: string): string
    return prefix .. ":" .. tostring(self.name)
end

local snapshot: Snapshot = Telemetry.new("café-東京")
snapshot.phase = "collecting"
Telemetry.increment(snapshot, "launches", 2)
Telemetry.increment(snapshot, "launches")
table.insert(snapshot.tags, "🚀")
table.insert(snapshot.tags, unicode)

local accumulator = fold(samples, 0, function(current: number, sample: number): number
    return current + clamp(sample, -1_000, 1_000)
end)

local attempts: number = 0
while attempts < 4 do
    attempts += 1
    if attempts == 2 then
        continue
    end
    counters.accepted += 1
end

repeat
    counters.rejected += 1
until counters.rejected >= 2

for index: number = 1, #samples do
    samples[index] *= 2
end

for key: string, value: number in pairs(snapshot.values) do
    if value >= 3 and key ~= "ignored" then
        metadata[key] = tostring(value)
    elseif not enabled or value == 0 then
        lastError = `empty key: {key}`
    else
        counters[key] = value
    end
end

local cursor: number = 0
do
    local limit = math.min(DEFAULT_LIMIT, #samples)
    while cursor < limit do
        cursor += 1
        if cursor > 10 then
            break
        end
    end
end

local untyped = require(script.Parent.Source)
local typedSource = untyped :: { read name: string, run: (number) -> boolean }
local optionalName = (typedSource.name :: string?)
local pair: Pair<number, number> = makePair(powered)
local first, second = pack(pair.first, pair.second)
local comparison = first <= second and second >= 0 or false
local unequal = first ~= decimal and first != exponent
local concatenated = single .. " | " .. double
concatenated ..= ` | {interpolated}`
quotient /= 2
floored //= 2
remainder %= 3
powered ^= 1
total -= tiny
total += binary

local frozen = table.freeze(table.clone(metadata))
local isFrozen = table.isfrozen(frozen)
local codepoints = { utf8.codepoint("東京", 1, -1) }
local safe, message = pcall(function(): string
    assert(comparison, "pair ordering failed")
    if not typedSource.run(cursor) then
        error("source rejected cursor", 0)
    end
    return formatSnapshot(snapshot, nestedInterpolation)
end)

snapshot.phase = if safe then "complete" else "failed"
local finalMessage = if safe then message else tostring(message)
local response: Result<string> = if safe
    then { ok = true, value = finalMessage }
    else { ok = false, error = finalMessage }

return {
    response = response,
    snapshot = snapshot,
    accumulator = accumulator,
    pair = pair,
    arithmetic = { quotient, floored, remainder, powered, total },
    strings = { single, double, escapes, longPlain, longEqual, concatenated },
    state = { attempts = attempts, cursor = cursor, frozen = isFrozen },
    unicode = { "café", "λ", "東京", "🚀", "𝌆", codepoints },
    constants = { Nothing = nil :: Nothing?, Anything = metadata :: Anything },
}
