-- Lua basic fixture with Unicode: café λ 🚀 𝌆
--[=[
Multiline comment containing keywords: local function return end.
Brackets and quotes remain harmless here: [[ "text" ]].
]=]

local banner = [==[
café λ spans a multiline string with astral glyphs 🚀 𝌆.
Nested-looking delimiters [[ stay literal in this string.
]==]

local function describe(name, ...)
  local values = { ... }
  local total = 0
  for _, value in ipairs(values) do
    total = total + value
  end
  if total > 10 then
    return string.format("%s: %d 🚀", name, total)
  else
    return name .. ": " .. banner
  end
end

print(describe("λ café 𝌆", 3, 4, 5))
