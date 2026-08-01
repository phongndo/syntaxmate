// Compact Power Query M fixture with café, 東京, and astral 🛰️ text.
section Basic;
shared Source = #table(
  type table [Name = text, Value = number],
  {{"alpha", 1}, {"東京", 2}, {"orbit 🛰️", 3}}
);
shared #"Filtered Rows" =
  Table.SelectRows(Source, each [Value] >= 2 and [Name] <> null);
shared MakeLabel = (name as text, optional suffix as nullable text) as text =>
  let
    safe = if suffix = null then "default" else suffix,
    escaped = "café#(tab)""quoted"""
  in
    escaped & name & ":" & safe;
shared Summary = [
  First = try #"Filtered Rows"{0}[Name] otherwise "missing",
  Range = {1..3},
  Hex = 0x2A,
  Ratio = 1.25E+2,
  Infinite = #infinity,
  NotNumber = #nan,
  Shared = #shared,
  Sections = #sections
];
Local = 42 meta [Documentation.Name = "answer"];
