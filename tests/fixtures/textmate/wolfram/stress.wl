#!/usr/bin/env wolframscript
(* Stress fixture for the Wolfram TextMate grammar.
   Unicode in comments: café 東京 λ 🚀 𝌆.
   (* Nested comments are legal, and this one is fully closed. *)
   The surrounding comment also closes on this line. *)

BeginPackage["Fixture`"];

transform::usage = "transform[data, opts] transforms a dataset.";
parse::usage = "parse[text] parses escaped and Unicode text.";
sampleData::usage = "sampleData is a compact association.";
transform::bad = "Cannot transform argument `1`.";

Options[transform] = {
  Method -> "Stable",
  WorkingPrecision -> MachinePrecision,
  PerformanceGoal -> "Quality"
};
Attributes[transform] = {Listable};
Format[sampleData] = Style["FixtureData", Bold];

Begin["`Private`"];

$fixtureVersion = "1.0.0";
$unicodeLabel = "café 東京 λ 🚀 𝌆";
$escapedString = "quote=\" slash=\\ tab=\t newline=\n return=\r";
$namedString = "alpha=\[Alpha] lambda=\[Lambda] degree=\[Degree]";
$encodedString = "BMP=\:03bb byte=\.41 astral=\|01F680 tetragram=\|01D306 octal=\101";
$continuedString = "continued \
text";

integers = {0, 1, 42, -17, +23};
binaryNumbers = {2^^0, 2^^101101, 2^^101.011, 2^^1*^10};
octalNumbers = {8^^7, 8^^755, 8^^17.4, 8^^7.1*^-3};
hexNumbers = {16^^a, 16^^DEADBEEF, 16^^cafe.babe, 16^^ff*^2};
decimalNumbers = {.125, 1., 3.14159, 6.022*^23, 9.1*^-31};
precisionNumbers = {1.234`20, 2.5`, 3.75`30*^4, 16^^ff`20};
accuracyNumbers = {1.234``10, 2.5``12*^-3, 2^^1.01``8};

formalSymbols = {\[FormalA], \[FormalAlpha], System`\[FormalB]};
namedCharacters = {\[Alpha], \[Beta], \[CapitalLambda], \[Pi], \[Infinity]};
encodedCharacters = {\:03bb, \.41, \|01F680, \101};
escapedPunctuation = {\ , \!, \%, \&, \(, \), \+, \-, \/, \@, \^, \_};

sampleData = <|
  "title" -> $unicodeLabel,
  "numbers" -> <|
    "binary" -> binaryNumbers,
    "octal" -> octalNumbers,
    "hex" -> hexNumbers,
    "decimal" -> decimalNumbers
  |>,
  "matrix" -> {{1, 2, 3}, {4, 5, 6}},
  "active" -> True,
  "missing" -> None
|>;

matrix = sampleData["matrix"];
firstRow = matrix[[1]];
lastColumn = matrix[[All, -1]];
middleParts = matrix[[1 ;; -1, 2 ;; 3]];
singlePart = sampleData[[Key["title"]]];

groupedArithmetic = (a + b) (c - d)/(e + f);
nestedGroups = ({(a + b), <|"inside" -> {c, d}|>}[[2]])["inside"];
linearSyntax = \(a + b*c - (d/e)\);
nestedLinearSyntax = \(outer + \(inner*2\)\);

identity = (# &);
square = (#^2 &);
weightedPair = (#1 + 2 #2 &);
namedSlot = (#value &);
allArguments = ({##} &);
argumentsAfterFirst = ({##2} &);
mixedSlots = ({#name, #2, ##, ##3} &);
operatorFunction = (x |-> x^3 - 1);

headAndTail[first_, rest__] := {first, {rest}};
allowEmpty[first_, rest___] := {first, {rest}};
defaulted[value_.] := Replace[value, None -> 0];
optional[value_: 10] := value;
typed[value_Integer] := value + 1;
tested[value_?NumericQ] := N[value];
conditioned[value_] /; value > 0 := Sqrt[value];
alternating[value : (True | False)] := Not[value];
repeatedPattern[patt : x..] := {patt};
nullableRepeated[patt : x...] := {patt};

transform[data_List, OptionsPattern[]] := Module[
  {method = OptionValue[Method], precision = OptionValue[WorkingPrecision]},
  Which[
    method === "Stable", N[Total[data], precision],
    method == "Fast", Total[data],
    True, Message[transform::bad, data]; $Failed
  ]
];

transform[data_Association, opts : OptionsPattern[]] :=
  transform[Values[data], opts];

parse[text_String] := StringCases[
  text,
  StartOfString ~~ prefix : LetterCharacter.. ~~ ":" ~~ rest___ ~~ EndOfString :>
    <|"prefix" -> prefix, "rest" -> rest|>
];

Context`Utility`normalize[x_] := Rescale[x];
Fixture`Private`qualified = Context`Utility`normalize[{2, 4, 8}];
System`Map[Fixture`Private`square, {1, 2, 3}];
Global`externalSymbol = Fixture`sampleData;
`relativePrivate = "relative context symbol";

mapped = square /@ Range[5];
deepMapped = square //@ {{1, 2}, {3, 4}};
applied = Plus @@ {1, 2, 3};
levelApplied = f @@@ {{a, b}, {c, d}};
prefixApplied = Reverse @ mapped;
postfixApplied = mapped // Total;
composition = (square @* Abs) /@ {-2, -1, 0, 1, 2};
rightComposition = (Abs /* square) /@ {-3, 3};

rules = {
  x_Integer -> x + 1,
  y_Real :> Round[y],
  HoldPattern[f[z_]] :> z^2
};
replacedOnce = {1, 2.2, f[3]} /. rules;
replacedRepeatedly = f[f[x]] //. f[t_] :> t;
selected = Select[Range[20], # > 5 && # <= 12 &];
logic = (True || False) && !False;
comparisons = {a < b, b <= c, c > d, d >= e, a != z, a =!= z};
identities = {a == a, a === a, Unequal[a, b]};

joinedText = "café" <> " / " <> "東京" <> " / λ 🚀 𝌆";
textPattern = StartOfString ~~ ("cat" | "dog") ~~ DigitCharacter... ~~ EndOfString;
textMatch = StringMatchQ["cat123", textPattern];
derivative = Sin'[x] + f''[x];
factorials = {5!, 7!!};
powers = x^2 + y^-3;
dotted = a.b + c ** d;

counter = 0;
counter++;
++counter;
counter--;
--counter;
counter += 5;
counter -= 2;
counter *= 3;
counter /= 2;
counter //= N;

scratch = 99;
scratch =.;
tag /: tagged[tag, value_] := Hold[value];
upExpression ^= "up-set value";
upDelayedExpression ^:= DateString[];

spanAll = Range[12][[;;]];
spanStep = Range[20][[2 ;; 18 ;; 4]];
sequence = a; b; c;
alternatives = Cases[{1, "x", 2.0}, _Integer | _Real];
patternTest = Cases[Range[10], _?EvenQ];

previousOutputs = {%, %%, %%%, %1, %42};
messageName = transform::usage;
Message[transform::bad, "not data"];

loadedConfig = << "fixture-config.wl";
loadedContext = << Fixture`Support`;
sampleData >> "fixture-output.wl";
sampleData >>> "fixture-output.log";

held = HoldComplete[
  loadedConfig,
  loadedContext,
  sampleData >> "held-output.wl",
  sampleData >>> "held-output.log"
];

stringEscapes = {
  "backspace=\b formfeed=\f newline=\n",
  "return=\r tab=\t quote=\" backslash=\\",
  "angle escapes=\<left\>right",
  "named=\[Rocket] encoded=\|01F680"
};

fileRules = FileNames["*.wl"] /. path_String :> FileBaseName[path];
associationMap = AssociationMap[square, Range[4]];
keyLookup = Lookup[associationMap, {1, 4}, Missing["KeyAbsent"]];
merged = Merge[{<|a -> 1|>, <|a -> 2, b -> 3|>}, Total];
nestedLookup = sampleData["numbers", "hex"];

table = Table[i^2 + j, {i, 1, 4}, {j, 0, 2}];
flattened = Flatten[table];
partitioned = Partition[flattened, 3];
summary = <|
  "minimum" -> Min[flattened],
  "maximum" -> Max[flattened],
  "mean" -> Mean[flattened],
  "count" -> Length[flattened]
|>;

finalResult = <|
  "data" -> sampleData,
  "summary" -> summary,
  "mapped" -> mapped,
  "text" -> joinedText,
  "escaped" -> stringEscapes,
  "previous" -> previousOutputs
|>;

End[];
EndPackage[];

Fixture`finalResult
