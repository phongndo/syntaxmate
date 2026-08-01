# TextMate stress fixture for the Nix language.
# Unicode in comments: café, λ, 中文, 日本語, 😀, 🚀.
/* A block comment can span lines.
   It contains punctuation: ${ } // ++ -> ? and "quotes".
   Keep the terminator visible and balanced. */

{ pkgs ? null
, lib ? null
, system ? "x86_64-linux"
, featureFlags ? { experimental = false; }
, ...
}@inputs:

let
  # Primitive literals and numeric forms.
  integer = 42; negative = -17; zero = 0;
  float = 3.14159; exponent = 6.022e23; tiny = 1.0e-9;
  enabled = true; disabled = false; nothing = null;

  # Identifiers may contain dashes, underscores, and apostrophes.
  kebab-name = "kebab";
  snake_name = "snake";
  foldl'Alias = builtins.foldl';

  # Double-quoted strings, escapes, and interpolations.
  greeting = "hello ${system}";
  escaped = "quote: \" slash: \\ newline:\n tab:\t return:\r";
  literalInterpolation = "this is not interpolation: \${ignored}";
  unicodeText = "café λ 中文 日本語 😀 🚀";
  computedText = "sum=${toString (integer + 8)}; enabled=${toString enabled}";
  adjacentText = greeting + " / " + unicodeText;

  # Indented strings preserve rich shell-like text without opening states.
  script = ''
    echo "system=${system}"
    echo 'single quotes are ordinary here'
    echo "literal interpolation: ''${HOME}"
    printf '%s\n' 'café λ 中文 😀'
    if [ -n "$PATH" ]; then
      echo "paths are available"
    fi
  '';

  indentedEscapes = ''
    A literal pair of single quotes: '''
    A dollar interpolation marker: ''${notEvaluated}
    Escaped control spelling: ''\n and ''\t
  '';

  # Path, lookup-path, and URI literals.
  relativePath = ./relative/path.nix;
  parentPath = ../shared/module.nix;
  absolutePath = /tmp/nix-stress/example.txt;
  lookupPath = <nixpkgs>;
  interpolatedPath = ./systems/${system}/default.nix;
  homepage = https://example.org/projects/nix;

  # Lists are whitespace separated and can contain arbitrary expressions.
  values = [ integer negative float true false null "text"
    (integer + 1) { name = "inline"; value = 7; }
  ];

  nestedLists = [ [ 1 2 ] [ 3 4 ] [] ];
  concatenated = [ 1 2 ] ++ [ 3 4 ] ++ values;

  # Attribute sets, recursion, dotted assignments, and quoted keys.
  base = {
    alpha = 1;
    beta = 2;
    nested.answer = 42;
    nested.message = greeting;
    "quoted-key" = "quoted value";
  };

  dynamicName = "generated-key";
  recursive = rec {
    first = 10;
    second = first + 5;
    deep.branch.leaf = second;
    ${dynamicName} = deep.branch.leaf;
    "${system}" = "dynamic interpolation in an attribute name";
  };

  source = {
    inherit system;
    packageName = "stress-demo";
    version = "1.2.3";
  };

  inherited = {
    inherit integer enabled greeting;
    inherit (source) packageName version;
  };

  merged = base // recursive // {
    beta = 200;
    extra = true;
  };

  # Selection, existence tests, and selection defaults.
  selected = base.nested.answer;
  selectedDefault = base.missing or "fallback";
  selectedDeepDefault = base.unknown.deep or null;
  hasAnswer = base ? nested.answer;
  hasQuoted = base ? "quoted-key";

  # Lambdas: simple arguments, destructuring, defaults, aliases, ellipsis.
  increment = x: x + 1;
  constant = _: greeting;
  choose = condition: yes: no: if condition then yes else no;
  formatter = { name, value ? 0, suffix ? "!", ... }:
    "${name}=${toString value}${suffix}";
  captureRight = { left, right ? left, ... }@all:
    { inherit left right all; };
  captureLeft = args@{ required, optional ? 9, ... }:
    args // { result = required + optional; };

  # Application, builtins, map, filter, and folds.
  incremented = map increment [ 1 2 3 4 ];
  doubled = builtins.map (x: x * 2) incremented;
  positives = builtins.filter (x: x > 0) [ (-2) 0 3 8 ];
  sum = builtins.foldl' (acc: x: acc + x) 0 doubled;
  product = builtins.foldl' builtins.mul 1 [ 2 3 4 ];
  names = builtins.attrNames merged;
  generated = builtins.listToAttrs (map
    (n: { name = "item-${toString n}"; value = n * n; })
    [ 1 2 3 ]);

  # Arithmetic, comparison, equality, and boolean operators.
  arithmetic = ((10 + 2) * 3 - 4) / 2;
  comparisons = {
    lt = 1 < 2; le = 2 <= 2;
    gt = 3 > 2; ge = 3 >= 3;
    eq = "same" == "same"; ne = 1 != 2;
  };
  booleanLogic = enabled && !disabled || featureFlags.experimental;
  implication = enabled -> (integer >= 0);

  # Control expressions can be nested and parenthesized.
  classified =
    if integer < 0 then "negative"
    else if integer == 0 then "zero"
    else "positive";

  guarded = assert integer > 0; increment integer;
  scoped = with builtins; concatStringsSep "," (map toString [ 1 2 3 ]);
  localScope = let
    a = 5;
    b = 7;
  in a * b;

  # Derivation-shaped data; this fixture is parsed, not evaluated.
  package = {
    pname = "syntax-stress";
    version = "0.1.0";
    src = relativePath;
    nativeBuildInputs = [ "tool-a" "tool-b" ];
    buildInputs = [ pkgs lib ];
    dontUnpack = true;
    configurePhase = ''
      echo "configuring ${source.packageName}"
    '';
    buildPhase = script;
    installPhase = ''
      mkdir -p "$out/share/syntax-stress"
      printf '%s\n' ${greeting} > "$out/share/syntax-stress/message"
    '';
    meta = {
      description = "A non-evaluated derivation-like attrset 😀";
      homepage = homepage;
      license = "example-only";
      platforms = [ system ];
    };
  };

in
assert hasAnswer;
with inherited;
{
  inherit arithmetic booleanLogic classified concatenated generated;
  inherit guarded implication localScope package product scoped;
  inherit selected selectedDefault sum unicodeText;

  formatted = formatter { name = packageName; value = integer; };
  callResult = captureLeft { required = 11; extra = "kept by ellipsis"; };
  conditional = choose (enabled && hasQuoted) "yes" "no";
}
