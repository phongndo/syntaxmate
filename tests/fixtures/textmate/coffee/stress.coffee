###
CoffeeScript TextMate stress fixture.
Exercises comments, strings, interpolation, regexps, calls, operators, and JSX.
Unicode BMP: café λ 東京. Astral: 🚀 𝌆.
@fixture stress
###

# Imports, exports, aliases, and reserved/control words.
import fs from 'node:fs'
import {readFile as read} from 'node:fs/promises'
export default class Reporter extends BaseReporter
  constructor: (@name = 'mark', options = {}) ->
    super options
    @enabled = options.enabled ? true

  report: (items...) =>
    return [] unless @enabled
    (format item, index for item, index in items when item?)

  format: (item, index) ->
    switch item.state
      when 'ready' then "#{index}: #{item.name} ✓"
      when 'paused' then 'waiting'
      else throw new Error "unknown state: #{item.state}"

# Function declarations and all parameter forms.
identity = (value) -> value
bound = (@value) => @value
defaults = (left = 1, right = 2, rest...) ->
  left + right + rest.length
'quoted-name': (x) -> x * x
"double-name": (x) => x / 2
inline = do -> 'called'

# Destructuring, objects, arrays, properties, and prototype access.
profile =
  name: 'Ada'
  active: on
  retries: 0
  nested:
    city: '東京'
    glyph: 'λ'
{name, active, nested: {city}} = profile
[head, middle..., tail] = [1, 2, 3, 4]
clone = {profile..., name: "#{name} 🚀"}
Reporter::version = '1.0'
instance = new Reporter name, enabled: yes
anonymous = new class Temporary

# Numeric forms and ranges.
decimal = 42
fraction = 3.1415
leading = .5
trailing = 2.
scientific = 6.02e23
hexadecimal = 0xCAFE
binary = 0b101010
octal = 0o755
inclusive = [0..10]
exclusive = [0...10]
selection = inclusive[2..5]

# Assignment, arithmetic, comparison, logical, and bitwise operators.
total = decimal + fraction - leading * trailing / 2 % 3
total += 1
total -= 2
total *= 3
total /= 4
total %= 5
mask = (hexadecimal & 0xff) | 0x10 ^ 0x02
mask <<= 1
mask >>= 1
mask >>>= 1
flag = active and total isnt 0 or not false
nativeFlag = active && total >= 1 || total < 100
same = name is clone.name
different = city != 'Paris'
exists = profile.missing?
fallback = profile.missing ? 'default'

# Single, double, heredoc, interpolation, escapes, and embedded JavaScript.
single = 'single \' quote and café'
double = "line\nhex \x41 octal \101 #{name} 東京"
literalBlock = '''
Single heredoc keeps #{name} literal.
Backslash escape: \n and astral 𝌆.
'''
interpolatedBlock = """
Hello #{name} from #{city}.
Nested result: #{identity "café 🚀"}.
"""
script = ``

# Line and block comments with annotation recognition.
# TODO: a line comment with #{} text and 🚀.
###
@param value documented annotation
FIXME: block comments end only at a matching delimiter.
###

# Regular expressions and heregexp internals.
simplePattern = /^(cat|dog)+\s?[0-9]+$/g
unicodePattern = /café|東京|🚀/i
herePattern = ///
  ^
  (?= prefix )
  (?: [a-z]+ | #{name} )
  [A-Z0-9_-]{1,8}
  \s+ \u6771
  $
///gimy
captured = herePattern.test "prefix#{name}A1 東京"

# Calls with and without parentheses, methods, built-ins, and globals.
parsed = parseInt '42', 10
finite = isFinite parsed
loaded = require './fixture'
delayed = setTimeout (-> console.info 'tick'), 10
maximum = Math.max decimal, parsed
rounded = Math.round fraction
keys = Object.keys profile
frozen = Object.freeze clone
arrayTest = Array.isArray inclusive
mapped = inclusive.map (n) -> n * n
filtered = mapped.filter((n) -> n % 2 is 0)
reduced = filtered.reduce ((sum, n) -> sum + n), 0
console.log maximum, rounded, keys, frozen, arrayTest, reduced

# Conditionals, loops, comprehensions, and exception handling.
status = if active then 'active' else 'idle'
status = 'disabled' unless instance.enabled
countdown = 3
while countdown > 0
  countdown--
until countdown >= 3
  countdown++
loop
  break if countdown > 3
  countdown++
for value in inclusive by 2
  continue unless value % 2 is 0
  console.debug value
for own key, value of profile
  console.info key, value
squares = (n * n for n in [1..5] when n isnt 3)

try
  read './missing.txt'
catch error
  console.error error
finally
  console.timeEnd 'fixture'

# Language constants, support values, and unary operators.
truths = [true, on, yes]
falses = [false, off, no]
nothing = null
specials = [Infinity, NaN, undefined]
kind = typeof profile
removed = delete profile.temporary
check = instance instanceof Reporter
context = [this, arguments]
paths = [module, exports, __filename, __dirname, global, process]

# Braces, punctuation, terminators, splats, and an intentional invalid id.
compact = {a: 1, b: 2}; joined = [a, b, head...]
123invalid
debugger

# JSX tags, attributes, expressions, fragments, and member names.
badge = <Badge tone="ready" count={inclusive.length} />
panel = <UI.Panel data-state='active'>
  <Header title="café 東京 🚀" />
  {badge}
  {squares.map (n) -> <Row key={n}>{n}</Row>}
</UI.Panel>

# Async/yield vocabulary and a final top-level call.
fetchLater = async (url) ->
  response = await fetch url
  response.json()
generator = ->
  yield head
  yield from middle
instance.report profile, clone
