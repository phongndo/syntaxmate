--- Tier B Luau basics: café, λ, 東京, 🚀, 𝌆.
--- @class Box
export type Box<T> = { value: T, label: string? }
type Mapper<T, U> = (item: T, index: number) -> U

type function OptionalOf(inner)
    return types.unionof(inner, types.singleton(nil))
end

@native
local function map<T, U>(items: {T}, visit: Mapper<T, U>): {U}
    local output: {U} = {}
    for index: number, item: T in ipairs(items) do
        output[index] = visit(item, index)
    end
    return output
end

local seed: number = 0x2A + 0b1010 + 1_000.5e-2
local greeting = "café λ" .. ' / 東京'
local note = [=[closed long string: 🚀 and 𝌆]=]
local boxed = ({ value = seed, label = greeting } :: Box<number>)
local rendered = `value={boxed.value}, note={note}`
return map({ seed, #rendered }, function(value: number): number
    return if value >= 1 and value ~= 13 then value // 2 else 0
end)
