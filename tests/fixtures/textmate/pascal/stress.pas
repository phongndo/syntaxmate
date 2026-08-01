program UnicodeAtlas;
{$mode objfpc}{$H+}
{$RANGECHECKS ON}
{$IFDEF DEBUG}
  {$DEFINE TRACE_ATLAS}
{$ENDIF}

uses
  SysUtils, Classes, Math;

const
  MaxPoints = 16;
  GoldenRatio = 1.61803398875;
  TinyScale = 2.5E-4;
  HexMask = $00FF00FF;
  WelcomeText = 'Café λ 漢字 🚀 𝄞';
  QuotedText = 'Pascal''s atlas';
  LineBreakCodes = #13#10;
  EnabledByDefault = True;

resourcestring
  SBadIndex = 'Point index is outside the route';

type
  TDirection = (North, East, South, West);
  TDirections = set of TDirection;
  TPointArray = array[0..MaxPoints - 1] of Double;

  TAtlasPoint = packed record
    X: Double;
    Y: Double;
    Caption: UnicodeString;
  end;

  generic TBox<T> = record
    Value: T;
  end;
  TIntegerBox = specialize TBox<Integer>;

  ERouteError = class(Exception);

  TRoute = class
  private
    FName: UnicodeString;
    FCount: Integer;
    FPoints: array[0..MaxPoints - 1] of TAtlasPoint;
    FVisible: Boolean;
    function GetPoint(Index: Integer): TAtlasPoint;
    procedure SetName(const AValue: UnicodeString);
  public
    constructor Create(const AName: UnicodeString);
    destructor Destroy; override;
    procedure AddPoint(AX, AY: Double; const ACaption: UnicodeString);
    function TotalDistance: Double;
    property Name: UnicodeString read FName write SetName;
    property Points[Index: Integer]: TAtlasPoint read GetPoint; default;
    property Visible: Boolean read FVisible write FVisible;
  end;

var
  Route: TRoute;
  Allowed: TDirections;
  BoxedCount: TIntegerBox;
  I: Integer;

procedure Trace(const Msg: UnicodeString); forward;

function Clamp(Value, LowBound, HighBound: Integer): Integer; inline;
begin
  if Value < LowBound then
    Result := LowBound
  else if Value > HighBound then
    Result := HighBound
  else
    Result := Value;
end;

procedure Trace(const Msg: UnicodeString);
begin
  {$IFDEF TRACE_ATLAS}
  WriteLn('[trace] ', Msg);
  {$ELSE}
  if Msg = '' then
    Exit;
  {$ENDIF}
end;

constructor TRoute.Create(const AName: UnicodeString);
begin
  inherited Create;
  FName := AName;
  FCount := 0;
  FVisible := EnabledByDefault;
end;

destructor TRoute.Destroy;
begin
  FName := '';
  inherited Destroy;
end;

procedure TRoute.SetName(const AValue: UnicodeString);
begin
  if AValue = '' then
    raise ERouteError.Create('A route needs a name');
  FName := AValue;
end;

function TRoute.GetPoint(Index: Integer): TAtlasPoint;
begin
  if (Index < 0) or (Index >= FCount) then
    raise ERouteError.CreateFmt('%s: %d', [SBadIndex, Index]);
  Result := FPoints[Index];
end;

procedure TRoute.AddPoint(AX, AY: Double; const ACaption: UnicodeString);
begin
  if FCount >= MaxPoints then
    raise ERouteError.Create('The route is full');
  with FPoints[FCount] do
  begin
    X := AX;
    Y := AY;
    Caption := ACaption;
  end;
  Inc(FCount);
end;

function TRoute.TotalDistance: Double;
var
  Index: Integer;
  DX, DY: Double;
begin
  Result := 0.0;
  for Index := 1 to FCount - 1 do
  begin
    DX := FPoints[Index].X - FPoints[Index - 1].X;
    DY := FPoints[Index].Y - FPoints[Index - 1].Y;
    Result := Result + Sqrt(DX * DX + DY * DY);
  end;
end;

function DirectionName(Direction: TDirection): String;
begin
  case Direction of
    North: Result := 'north';
    East:  Result := 'east';
    South: Result := 'south';
    West:  Result := 'west';
  else
    Result := 'unknown';
  end;
end;

procedure ExerciseOperators;
var
  Bits, Quotient, Remainder: Integer;
  Ready: Boolean;
begin
  Bits := (HexMask and $FFFF) xor (1 shl 4);
  Bits := Bits or (Bits shr 2);
  Quotient := Bits div 7;
  Remainder := Bits mod 7;
  Ready := not False and ((Quotient + Remainder) >= 0);
  if Ready and (East in Allowed) then
    Trace(DirectionName(East));
end;

procedure WalkBackwards;
var
  Index: Integer;
begin
  for Index := Route.FCount - 1 downto 0 do
  begin
    if Index = 2 then
      Continue;
    if Index < 0 then
      Break;
    Trace(Route[Index].Caption);
  end;
end;

procedure CountDown;
var
  Counter: Integer;
begin
  Counter := 3;
  while Counter > 0 do
  begin
    Dec(Counter);
  end;
  repeat
    Inc(Counter);
  until Counter = 2;
end;

{$IFDEF CPUX86_64}
procedure CpuNoOp; assembler; nostackframe;
asm
  nop
end;
{$ENDIF}

begin
  // Line comment: naïve façade, Ελληνικά, and astral 🧭.
  { Brace comment with BMP Ж and astral 🌍; it closes here. }
  (* Parenthesized comment mentions 東京 and U+1D11E 𝄞. *)
  Allowed := [North, East, West];
  BoxedCount.Value := Clamp(3 + 5 * 2, 0, MaxPoints);
  Route := TRoute.Create(WelcomeText + LineBreakCodes + QuotedText);
  try
    Route.AddPoint(0, 0, 'origin');
    Route.AddPoint(GoldenRatio, TinyScale, 'α point');
    Route.AddPoint(3.0, 4.0, 'emoji 🚀');
    ExerciseOperators;
    WalkBackwards;
    CountDown;
    for I := 0 to BoxedCount.Value - 1 do
      if I < Route.FCount then
        WriteLn(I:2, ' ', Route[I].Caption);
    Assert(Route.TotalDistance >= 0.0);
    {$IFDEF CPUX86_64}
    CpuNoOp;
    {$ENDIF}
  except
    on E: ERouteError do
      WriteLn('Route error: ', E.Message);
    on E: Exception do
      WriteLn('Unexpected: ', E.ClassName, ': ', E.Message);
  end;
  try
    if Route is TRoute then
      WriteLn(Route.Name, ' distance=', Route.TotalDistance:0:3);
  finally
    Route.Free;
  end;
end.
