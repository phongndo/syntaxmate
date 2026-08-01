program BasicPascal;
{$mode objfpc}{$H+}

uses SysUtils;

const Greeting = 'Café λ 漢字 🚀';
type
  TShade = (Light, Medium, Dark);
  TShades = set of TShade;
  TSample = record Name: String; Value: Integer; end;

function Doubled(N: Integer): Integer;
begin
  Result := N * 2;
end;

var Sample: TSample;
begin
  { BMP Ω and astral 𝄞 in a closed comment. }
  Sample.Name := Greeting + #32 + 'Pascal''s sample';
  Sample.Value := Doubled($15);
  if Sample.Value > 0 then
    WriteLn(Sample.Name, ': ', Sample.Value);
end.
