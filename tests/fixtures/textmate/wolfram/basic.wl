#!/usr/bin/env wolframscript
(* Basic Wolfram fixture: café 東京 λ 🚀 𝌆. *)
(* An outer comment (* with a nested comment *) closes here. *)
Demo`scale::usage = "scale[x, factor] scales x; quote: \"ok\" and newline:\n";
Options[Demo`scale] = {Method -> "Fast", WorkingPrecision -> MachinePrecision};
Demo`scale[x_?NumericQ, factor_: 2] := Module[{value = x factor}, value];
Demo`data = <|"name" -> "café 東京 λ 🚀 𝌆", "values" -> {1, 2^^1011, 8^^17, 16^^ff}|>;
Demo`reals = {0, -42, .5, 3., 6.02*^23, 1.25`20, 2.5``10};
Demo`escaped = {\[FormalAlpha], \[Pi], \[Degree], \:03bb, \.41, \|01F680};
Demo`text = "tab:\t slash:\\ named:\[Lambda] astral:\|01D306";
Demo`matrix = {{a, b}, {c, d}};
Demo`column = Demo`matrix[[All, 2]];
Demo`linear = \(a + (b*c)\);
Demo`pure = (#^2 + #2 &);
Demo`slots = ({#name, #2, ##, ##3} &);
Demo`patterns[x_, ys__, rest___] := {x, ys, rest};
Demo`previous = {%, %%, %12};
Demo`mapped = System`Map[Demo`scale[#, 3] &, Demo`data["values"]];
Demo`message::bad = "Bad input `1`";
Demo`rule = HoldPattern[f[x_.]] :> (x /. None -> 1);
Demo`result = (Demo`mapped // Total) /. n_ /; n > 0 :> n;
