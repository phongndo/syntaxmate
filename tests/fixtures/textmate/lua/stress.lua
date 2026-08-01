-- Lua stress fixture: café, λ, 中文, and an astral emoji 😀.
-- Single-line comments exercise punctuation, operators + - * /, and keywords.
--[=[
Long comment with balanced-looking text that is not code:
  function phantom(...) return "未执行 🚀" end
  Delimiters with fewer equals do not close this block: [[ text ]]
]=]

local module = {}
local truth, falsity, nothing = true, false, nil
local decimal = 12345
local fraction = 12.75
local leading = .5
local exponent = 6.022e23
local signed_exponent = 1.5E-10
local hex_integer = 0xDEADbeef
local hex_fraction = 0x1.fp+4
local tiny_hex = 0x1p-8

local single = 'single quoted: \'ok\''
local double = "double quoted: \"ok\""
local controls = "\a\b\f\n\r\t\v\\"
local numeric_escapes = "\065\066\067 \x44\u{45}"
local spaced = "before\z
    after"
local continued = "first line\
second line"
local unicode = "café λ 中文 😀"
local long_plain = [[
Long string: quotes ' " and -- are literal here.
Unicode remains literal: naïve, Ελληνικά, 日本語, 🐍.
]]
local long_equal = [==[
An equals-delimited long string can contain [[nested-looking]] text.
It can also contain [=[ another delimiter ]=] without ending.
]==]

local array = { 10, 20, 30; 40 }
local mixed = {
  name = "fixture",
  ["hyphen-key"] = true,
  [1 + 1] = "computed",
  nested = { x = 1, y = 2 },
  trailing = "comma",
}
mixed.extra = nothing or "fallback"
mixed["count"] = #array

local function identity(value)
  return value
end
local function arithmetic(a, b)
  local sum = a + b
  local difference = a - b
  local product = a * b
  local quotient = a / b
  local floor_quotient = a // b
  local remainder = a % b
  local power = a ^ b
  return sum, difference, product, quotient, floor_quotient, remainder, power
end
function module.pack(tag, ...)
  local values = { ... }
  return { tag = tag, count = select("#", ...), values = values }
end
function module:describe(prefix, ...)
  local suffixes = table.concat({ ... }, ",")
  return prefix .. ":" .. self.name .. ":" .. suffixes
end
module.name = "stress"
local anonymous = function(x)
  return function(y)
    return x + y
  end
end
local add_five = anonymous(5)

local precedence = (decimal + fraction * 2 ^ 3) // 4 % 7
local comparison = decimal < exponent and fraction >= leading
local equality = truth == not falsity and nothing ~= 0
local concatenated = single .. " | " .. double
local length = #concatenated
local bitwise = (0xF0 & 0xCC) | (0x0F ~ 0x03)
local shifted = (bitwise << 2) >> 1
local complemented = ~shifted

if comparison and equality then
  mixed.status = "both true"
elseif comparison or falsity then
  mixed.status = "one true"
else
  mixed.status = "neither"
end
local countdown = 3
while countdown > 0 do
  countdown = countdown - 1
  if countdown == 1 then
    break
  end
end
repeat
  countdown = countdown + 1
until countdown >= 3
local numeric_total = 0
for index = 1, 10, 2 do
  numeric_total = numeric_total + index
end
local visited = {}
for key, value in pairs(mixed) do
  visited[#visited + 1] = tostring(key) .. "=" .. tostring(value)
end
for index, value in ipairs(array) do
  array[index] = value * 2
end
do
  local shadow = "block local"
  mixed.shadow = shadow
end
local state = 0
::again::
state = state + 1
if state < 2 then
  goto again
end
::finished::

local Point = {}
Point.__index = Point
function Point.new(x, y)
  return setmetatable({ x = x, y = y }, Point)
end
function Point:magnitude_squared()
  return self.x * self.x + self.y * self.y
end
function Point:__tostring()
  return ("Point(%g, %g)"):format(self.x, self.y)
end
function Point.__add(left, right)
  return Point.new(left.x + right.x, left.y + right.y)
end

local p = Point.new(3, 4)
local q = Point.new(-1, 2)
local r = p + q
local point_text = tostring(r)

local proxy_store = {}
local proxy = setmetatable({}, {
  __index = proxy_store,
  __newindex = function(_, key, value)
    proxy_store[key] = value
  end,
})
proxy.answer = 42
local function producer(limit)
  return coroutine.create(function(seed)
    local current = seed
    for _ = 1, limit do
      current = current + 1
      coroutine.yield(current)
    end
    return "done"
  end)
end
local worker = producer(2)
local resumed, first = coroutine.resume(worker, 10)
local resumed_again, second = coroutine.resume(worker)
local final_resume, final_value = coroutine.resume(worker)
local worker_status = coroutine.status(worker)

local ok, result = pcall(function()
  assert(add_five(7) == 12, "unexpected sum")
  return identity("safe")
end)
local handled, message = xpcall(function()
  error("intentional café failure", 0)
end, function(err)
  return "handled: " .. err
end)

module.summary = {
  numbers = { decimal, fraction, exponent, hex_fraction },
  strings = { controls, numeric_escapes, spaced, continued, unicode },
  long_strings = { long_plain, long_equal },
  operators = { precedence, bitwise, shifted, complemented, length },
  flow = { numeric_total, state, visited },
  coroutine = { resumed, first, resumed_again, second, final_resume, final_value },
  protected = { ok, result, handled, message, worker_status },
  point = { object = r, text = point_text, magnitude = p:magnitude_squared() },
  proxy = proxy.answer,
}
return module
