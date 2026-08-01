// Power Query M stress fixture: broad lexical closure and Unicode 🚀.
/* A block comment may contain let, #shared, 1..9, and "fake strings". */
section Observatory;

shared Seed = #table(
  type table [Name = text, Amount = number, Active = logical, Seen = date],
  {
    {"alpha", 1, true, #date(2025, 1, 1)},
    {"café", 2.5, false, #date(2025, 1, 2)},
    {"東京", 3.75, true, #date(2025, 1, 3)},
    {"rocket 🚀", 4, true, #date(2025, 1, 4)}
  }
);

shared Literals = [
  Null = null,
  True = true,
  False = false,
  Integer = 42,
  Decimal = .625,
  Exponent = 6.022E+23,
  SignedExponent = 1e-9,
  Hexadecimal = 0xDEADBEEF,
  PositiveInfinity = #infinity,
  NotANumber = #nan,
  Text = "quote: ""M""; café; 東京; 🚀",
  Escapes = "line#(cr,lf)tab#(tab)hash#(#)lambda#(03BB)rocket#(0001F680)"
];

shared Temporal = [
  Date = #date(2025, 6, 30),
  Time = #time(12, 34, 56),
  DateTime = #datetime(2025, 6, 30, 12, 34, 56),
  Zone = #datetimezone(2025, 6, 30, 12, 34, 56, 2, 0),
  Duration = #duration(1, 2, 3, 4),
  Binary = Binary.FromText("AQIDBA==", BinaryEncoding.Base64)
];

shared #"Quoted "" Member" = [
  #"field with spaces" = "value",
  #"Unicode λ" = "Ελληνικά",
  Normal = 1
];

shared Arithmetic =
  let
    Add = 1 + 2,
    Subtract = 9 - 3,
    Multiply = 6 * 7,
    Divide = 84 / 2,
    Compared = Add = Subtract or Multiply <> Divide,
    Ordered = Add < Multiply and Divide >= Subtract and Add <= Divide,
    Combined = "left" & "right"
  in
    [Add = Add, Subtract = Subtract, Product = Multiply, Quotient = Divide,
     Compared = Compared, Ordered = Ordered, Combined = Combined];

shared Transform =
  (input as table, optional minimum as nullable number) as table =>
  let
    floor = if minimum = null then 0 else minimum,
    selected = Table.SelectRows(
      input,
      each [Amount] >= floor and [Active] = true
    ),
    labeled = Table.AddColumn(
      selected,
      "Label",
      each [Name] & ":" & Number.ToText([Amount]),
      type text
    )
  in
    labeled;

shared SafeTransform = (value as any) as any =>
  try Transform(value, 2) otherwise #table({}, {});

shared Factorial = (n as number) as number =>
  if n <= 1 then 1 else n * @Factorial(n - 1);

shared CollectionOperators =
  let
    InclusiveRange = {1..8},
    CombinedLists = {1, 2} & {3, 4},
    CombinedRecords = [A = 1] & [B = 2],
    First = InclusiveRange{0},
    OptionalItem = InclusiveRange{99}?,
    OptionalField = CombinedRecords[Missing]?,
    RequiredField = CombinedRecords[A]
  in
    [Range = InclusiveRange, Lists = CombinedLists, Records = CombinedRecords,
     First = First, MaybeItem = OptionalItem, MaybeField = OptionalField,
     Required = RequiredField];

shared TypeSamples =
  let
    AnyType = type any,
    AnyNonNullType = type anynonnull,
    BinaryType = type binary,
    DateType = type date,
    DateTimeType = type datetime,
    ZoneType = type datetimezone,
    DurationType = type duration,
    FunctionType = type function,
    ListType = type list,
    LogicalType = type logical,
    NoneType = type none,
    NullType = type null,
    NumberType = type number,
    RecordType = type record,
    TableType = type table,
    TextType = type text,
    TypeType = type type,
    ActionType = type action,
    OpenRecord = type [Name = text, optional Amount = nullable number, ...],
    OpenList = type {number, ...}
  in
    {AnyType, AnyNonNullType, BinaryType, DateType, DateTimeType, ZoneType,
     DurationType, FunctionType, ListType, LogicalType, NoneType, NullType,
     NumberType, RecordType, TableType, TextType, TypeType, ActionType,
     OpenRecord, OpenList};

shared Metadata =
  Transform meta [
    Documentation.Name = "Transform",
    Documentation.LongDescription = "Filters observatory rows 🚀",
    Documentation.Examples = {[Description = "basic", Code = "Transform(Seed, 2)"]}
  ];

shared Introspection =
  let
    AllSections = #sections,
    AllShared = #shared,
    Current = AllSections[Observatory]?,
    Names = Record.FieldNames(AllShared),
    HasSeed = Record.HasFields(AllShared, "Seed")
  in
    [Sections = AllSections, Shared = AllShared, Current = Current,
     Names = Names, HasSeed = HasSeed];

shared TypedPredicates = (value as any) as logical =>
  value is number or value is text or value is null;

shared ErrorHandling = (record as record) as any =>
  let
    demanded = try record[Required] otherwise error "Required field missing",
    recovered = try demanded[Value] otherwise [Value = null]
  in
    recovered;

shared EachForms = [
  Names = List.Transform({[Name = "Ada"], [Name = "李雷"]}, each [Name]),
  Doubled = List.Transform({1, 2, 3}, each _ * 2),
  Kept = List.Select({1, 2, 3, 4}, each not (_ < 3))
];

shared NestedRecords = [
  Data = [Base = 100, Rate = 1.8],
  Progression = Data[Base] * Data[Rate],
  Path = [Outer = [Inner = [Value = "deep"]]],
  Selected = Path[Outer][Inner][Value]
];

shared InvokeMany =
  let
    Functions = {Number.Abs, Number.Round, Number.Sign},
    Results = List.Transform(Functions, each _(-3.5)),
    Zipped = List.Zip({{"abs", "round", "sign"}, Results})
  in
    Zipped;

shared Tables =
  let
    Added = Table.AddIndexColumn(Seed, "Index", 0, 1, Int64.Type),
    Renamed = Table.RenameColumns(Added, {{"Amount", "Value"}}),
    Sorted = Table.Sort(Renamed, {{"Value", Order.Descending}}),
    Grouped = Table.Group(Sorted, {"Active"}, {{"Rows", each _, type table}}),
    Expanded = Table.ExpandTableColumn(Grouped, "Rows", {"Name", "Value"})
  in
    Expanded;

shared Generator = List.Generate(
  () => [Index = 0, Value = 1],
  each [Index] < 8,
  each [Index = [Index] + 1, Value = [Value] * 2],
  each [Value]
);

shared Accumulated = List.Accumulate(
  {1, 2, 3, 4, 5},
  0,
  (state as number, current as number) as number => state + current
);

shared Query =
  let
    Clean = Transform(Seed, 1.5),
    WithRank = Table.AddIndexColumn(Clean, "Rank", 1, 1),
    Result = Table.SelectColumns(WithRank, {"Rank", "Name", "Label"})
  in
    Result;

section Consumer;
shared Imported = Observatory!Query;
shared ImportedFunction = Observatory!Transform;
shared Result = ImportedFunction(Observatory!Seed, 2);
shared Reflection = Observatory!Introspection;
LocalFirst = try Result{0}[Name] otherwise "none";
