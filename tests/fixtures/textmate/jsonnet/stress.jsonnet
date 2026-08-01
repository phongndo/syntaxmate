// Jsonnet TextMate stress fixture, written to cover every repository rule.
// BMP samples: café, naïve, λ, Ω, 雪, 東京, 中文.
// Astral samples: 🚀, 😀, 𝄞, 𐐷, 𝌆.
/* The root block comment spans lines and contains operators + - * / %.
   String-like text "not a string" and std.length() remain comments.
   The terminator is intentionally isolated for state coverage.
*/
# Hash comments are line matches but use comment.block.jsonnet.

local identity(value) = value;
local pair(left, right) = { left: left, right: right };
local decorate(text, suffix) = text + suffix;
local choose(flag, yes, no) = if flag then yes else no;
local importedLibrary = import "fixtures/library.libsonnet";
local importedBanner = importstr "fixtures/banner.txt";
local rootObject = $;
local plainFunction = function(x) x * x;

{
  // Bare fields exercise :, ::, ::: and their additive + forms.
  publicField: true,
  publicSpaced   : false,
  additivePublic+: 1,
  additivePublicSpaced +: 2,
  hiddenField:: "hidden",
  hiddenSpaced   :: "hidden with spaces",
  additiveHidden+:: [1, 2],
  additiveHiddenSpaced +:: [3, 4],
  forcedField::: "forced",
  forcedSpaced   ::: "forced with spaces",
  additiveForced+::: { enabled: true },
  additiveForcedSpaced +::: { enabled: false },
  quotedField: { "quoted-key": true, 'single-key': false },
  unicodeField: { 東京: "colon alone is still an operator" },

  // Language constants and every numeric pattern shape.
  constants: [true, false, null],
  integerZero: 0,
  integerLarge: 987654321,
  exponentUpper: 6E23,
  exponentSigned: 12E-3,
  exponentPlus: 9e+7,
  decimal: 3.14159,
  decimalTrailingDot: 10.,
  decimalExponent: 2.5e-8,
  leadingDot: .125,
  leadingDotExponent: .75E+2,
  negativeInteger: -42,
  negativeFraction: -.5,
  boundaryWords: [trueValue, falsehood, nullish, value123],

  // Double strings: all recognized escapes, illegal escapes, and boundaries.
  doublePlain: "double quoted ASCII",
  doubleUnicode: "BMP café λ 雪 東京; astral 🚀 😀 𝄞 𐐷",
  doubleEscapesA: "quote: \" slash: \/ backslash: \\",
  doubleEscapesB: "backspace: \b formfeed: \f newline: \n",
  doubleEscapesC: "return: \r tab: \t lambda: \u03BB snow: \u96EA",
  doubleAstralBeforeEscape: "🚀 then escaped tab \t and quote \" done",
  doubleIllegalQ: "illegal \q remains inside the string",
  doubleIllegalX: "illegal \x41 and illegal \z",
  doubleIncompleteUnicode: "not recognized as an escape: \u12",

  // Single strings share the grammar's double-quoted scope name.
  singlePlain: 'single quoted ASCII',
  singleUnicode: 'BMP déjà vu Ω 中文; astral 🚀 𝌆',
  singleEscapesA: 'quote: \' slash: \/ backslash: \\',
  singleEscapesB: 'backspace: \b formfeed: \f newline: \n',
  singleEscapesC: 'return: \r tab: \t lambda: \u03BB',
  singleAstralBeforeEscape: '😀 then escaped quote \' and tab \t done',
  singleIllegalQ: 'illegal \q remains inside the string',
  singleIllegalX: 'illegal \x41 and illegal \z',
  singleIncompleteUnicode: 'not recognized as an escape: \u12',

  // Every operator character in [-!%&*+/:<=>^|~] appears below.
  arithmetic: 1 + 2 - 3 * 4 / 5 % 2,
  unary: -1 + +2,
  comparisonsA: [1 < 2, 2 <= 2, 3 > 2, 3 >= 3],
  comparisonsB: [1 == 1, 1 != 2],
  logical: true && false || !false,
  bitwise: (5 & 3) | (8 ^ 2),
  complement: ~15,
  shifts: [1 << 4, 32 >> 2],
  assignmentLike: a = b,
  colonOperatorOnly: 東京: true,
  dollarReference: $.publicField,
  rootReference: rootObject,

  // Keyword.other and keyword.control matches outside function meta scopes.
  selfReference: self.publicField,
  superReference: super.publicField,
  importValue: importedLibrary,
  importString: importedBanner,
  localValue: (local temporary = 7; temporary),
  strictCall: identity(9) tailstrict,
  conditional: if true then "yes" else "no",
  asserted: assert true; "assert passed",
  comprehension: [x * x for x in [1, 2, 3] if x > 1],
  objectComprehension: { ["item" + std.toString(x)]: x for x in [1, 2] },
  explicitError: if false then null else error "intentional grammar probe",
  functionValue: plainFunction,
  bareFunctionKeywordProbe: function value,

  // Built-ins: each alternative in builtin-functions is represented.
  trigAcos: std.acos(1),
  trigAsin: std.asin(0),
  trigAtan: std.atan(1),
  roundCeil: std.ceil(1.2),
  charFromCode: std.char(955),
  codeFromChar: std.codepoint("λ"),
  trigCos: std.cos(0),
  exponential: std.exp(1),
  floatExponent: std.exponent(8.0),
  filtered: std.filter(function(x) x > 0, [-1, 0, 1]),
  roundFloor: std.floor(1.8),
  forced: std.force({ value: 1 }),
  measured: std.length("café 🚀"),
  logarithm: std.log(10),
  madeArray: std.makeArray(4, function(i) i * i),
  floatMantissa: std.mantissa(12.5),
  fieldsOfObject: std.objectFields({ a: 1, b:: 2 }),
  hasField: std.objectHas({ a: 1 }, "a"),
  powered: std.pow(2, 8),
  trigSin: std.sin(0),
  squareRoot: std.sqrt(81),
  trigTan: std.tan(0),
  valueType: std.type([1, 2]),
  currentFile: std.thisFile,
  absolute: std.abs(-7),
  equality: std.assertEqual({ a: 1 }, { a: 1 }),
  bashEscaped: std.escapeStringBash("café 🚀"),
  dollarsEscaped: std.escapeStringDollars("$HOME"),
  jsonEscaped: std.escapeStringJson("λ\n🚀"),
  pythonEscaped: std.escapeStringPython("it's fine"),
  filterMapped: std.filterMap(function(x) x > 1, function(x) x * 2, [1, 2, 3]),
  flattened: std.flattenArrays([[1, 2], [3], []]),
  foldedLeft: std.foldl(function(acc, x) acc + x, [1, 2, 3], 0),
  foldedRight: std.foldr(function(x, acc) x + acc, [1, 2, 3], 0),
  formatted: std.format("%s:%d", ["port", 8080]),
  joined: std.join(",", ["café", "東京", "🚀"]),
  lineArray: std.lines(["first", "second"]),
  iniManifest: std.manifestIni({ sections: { main: { key: "value" } } }),
  pythonManifest: std.manifestPython({ greeting: "héllo" }),
  pythonVarsManifest: std.manifestPythonVars({ launch: "🚀" }),
  mapped: std.map(function(x) x + 1, [1, 2, 3]),
  maximum: std.max(8, 13),
  minimum: std.min(8, 13),
  modulo: std.mod(17, 5),
  plainSet: std.set([3, 1, 2]),
  setDifference: std.setDiff([1, 2, 3], [2]),
  setIntersection: std.setInter([1, 2], [2, 3]),
  setMembership: std.setMember(2, [1, 2, 3]),
  setUnioned: std.setUnion([1, 2], [2, 3]),
  sorted: std.sort([3, 1, 2]),
  ranged: std.range(1, 5),
  splitText: std.split("a::b::c", "::"),
  characters: std.stringChars("λ🚀"),
  substring: std.substr("café🚀", 1, 4),
  stringified: std.toString({ snow: "雪" }),
  uniqueValues: std.uniq([1, 1, 2, 3, 3]),
  builtinBoundary: std.lengthExtra,

  // Named calls activate meta.function and recursively include expressions.
  simpleCall: identity("café 🚀"),
  pairedCall: pair(true, null),
  decoratedCall: decorate("launch", " 🚀"),
  selectedCall: choose(false, "left", "right"),
  nestedCalls: decorate(identity("東京"), choose(true, " 🚀", " 😀")),
  multilineCall: decorate(
    identity("BMP λ followed by astral 𝄞"),
    choose(
      true,
      " / selected",
      " / ignored"
    )
  ),
  commentedCall: pair(
    /* comment inside meta.function */ true,
    identity("value") // line comment inside the call
  ),

  // Triple strings have no nested escape, comment, or keyword patterns.
  tripleDocument: |||
    Triple line one: café λ 雪.
    Triple line two: astral 🚀 😀 𝄞 𐐷.
    Text stays triple-scoped: true null std.length("x") \q.
    Comment markers are plain text here: // hash # block /* still text */.
  |||,
  tripleCompact: |||one-line triple with 東京 and 𝌆|||,

  commentsAroundValues: [
    true, // slash comment with café and 🚀
    false, # hash comment with λ and 😀
    /* block comment begins after an item
       and continues with 雪, 𝄞, operators && ||,
       then closes before the next literal */ null,
  ],

  // These final values ensure token boundaries occur after astral characters.
  astralThenEscape: "🚀𝄞 before \n after",
  astralThenBuiltin: "😀" + std.length([1, 2, 3]),
  astralThenKeyword: "𐐷" + (if true then "ok" else "no"),
  finalValue: decorate("complete", " ✓"),
}
