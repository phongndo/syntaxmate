# Nix basic fixture with Unicode: café λ 🚀 𝌆
/* A multiline comment includes operators such as //, ++, and ->.
   Delimiter-like text and "quotes" are fully contained here. */
{ who ? "world", enabled ? true }:
let
  label = "café λ";
  greeting = "Hello, ${who}: café λ 🚀 𝌆";
  escaped = "literal interpolation: \${who}";
  script = ''
    echo "${greeting}"
    echo "literal marker: ''${HOME}"
  '';
  values = [ 1 2 3 5 ];
  doubled = builtins.map (value: value * 2) values;
  config = rec {
    inherit enabled;
    nested.answer = 42;
    "café-key" = greeting;
  };
  result = if enabled then doubled else [ ];
in
{
  inherit label greeting escaped result;
  answer = config.nested.answer;
  missing = config.optional or "fallback";
  shell = script;
};
