#light "on"
#nowarn "40"
#if DEBUG
#nowarn "52"
#else
#nowarn "57"
#endif

namespace TextMate.FSharpFixtures
open System
open System.Collections.Generic
open Microsoft.FSharp.Quotations
module IO = System.IO

(**
# Grammar stress fixture
Exercises **Markdown**, `code`, [links](https://fsharp.org), and Unicode:
λ, 東京, 🚀, and 𝌆.
*)
module internal Stress =
    #if DEBUG
    let buildMode = "debug"
    #else
    let buildMode = "release"
    #endif

    [<Literal>]
    let Rocket = "🚀"
    [<Measure>]
    type m
    [<Measure>]
    type s
    [<CLIMutable>]
    type Vector =
        { X: float<m>
          Y: float<m>
          Label: string }
    type Shape<'T> =
        | Empty
        | Point of Vector
        | Circle of center: Vector * radius: float<m>
        | Tagged of label: string * payload: 'T
    [<Struct>]
    type Token =
        | Word of text: string
        | End
    type Color =
        | Red = 1
        | Green = 2
        | Blue = 4

    exception ParseFailure of input: string * position: int
    type Transformer<'T> = delegate of 'T -> 'T
    type IDescribe =
        abstract member Describe: prefix: string -> string
    type Counter(initial: int) =
        let mutable value = initial
        member _.Value
            with get () = value
            and set next = value <- next
        member _.Increment(?amount: int) =
            value <- value + defaultArg amount 1
            value
        interface IDescribe with
            member _.Describe prefix = sprintf "%s%d" prefix value
        override _.ToString() = sprintf "Counter(%d)" value
    [<AbstractClass>]
    type Entity(name: string) =
        member _.Name = name
        abstract member Kind: string
        default _.Kind = "entity"
    type Person(name: string, age: int) =
        inherit Entity(name)
        member _.Age = age
        override _.Kind = "person"
    type Box<'T when 'T: equality>(value: 'T) =
        member _.Value = value
        member _.Same(other: 'T) = value = other
        override _.GetHashCode() = hash value
    type System.String with
        member this.WordCount =
            this.Split([| ' '; '\t'; '\n' |], StringSplitOptions.RemoveEmptyEntries).Length
    /// Total active pattern for parity.
    let (|Even|Odd|) number =
        if number % 2 = 0 then Even else Odd
    /// Partial active pattern around Int32.TryParse.
    let (|Int|_|) (text: string) =
        match Int32.TryParse text with
        | true, value -> Some value
        | false, _ -> None
    let inline (|+|) left right = left + right
    let (<!>) mapper value = mapper value
    let inline combine (left: ^T) (right: ^U) : ^V =
        ((^T or ^U) : (static member (+) : ^T * ^U -> ^V) (left, right))
    let λ = 42
    let 東京 = "東京"
    let ``name with spaces`` = "double ticks"
    let decimalInteger = 1_000_000
    let negativeInteger = -42
    let hexadecimal = 0xDEAD_BEEFu
    let octal = 0o755
    let binary = 0b1010_0110uy
    let int64Value = 9_223_372_036_854_775_000L
    let nativeValue = 123n
    let floatValue = 6.022_140_76e23
    let singleValue = 3.14159f
    let decimalValue = 12.50M
    let distance = 1_500.0<m>
    let duration = 125.0<s>
    let speed = distance / duration
    let acceleration = 9.81<m/s^2>
    let escaped = "line one\nline two\t\u03BB \U0001F680 \U0001D306"
    let verbatim = @"C:\fixtures\東京\""quoted""\file.fsx"
    let byteText = "bytes"B
    let λChar = 'λ'
    let newlineChar = '\n'
    let quoteChar = '\''
    let triple =
        """A triple-quoted string
        keeps "quotes", %A, 東京, 🚀,
        and the astral symbol 𝌆 intact.
        """
    let Tokyo = 東京
    let formatted =
        sprintf "int=%04d hex=%08X float=%0.2f any=%A object=%O" 12 255 3.5 Tokyo Rocket

    let anonymous =
        {| Name = 東京
           CodePoint = 0x6771
           Payload = Some Rocket |}
    let moved =
        { X = 3.0<m>
          Y = 4.0<m>
          Label = "origin → moved" }
    let describeNumber input =
        match input with
        | Int value when value < 0 -> "negative"
        | Int (Even as value) -> sprintf "even %d" value
        | Int (Odd as value) -> sprintf "odd %d" value
        | "" | null -> "missing"
        | other -> raise (ParseFailure(other, 0))
    let describeShape shape =
        match shape with
        | Empty -> "empty"
        | Point { X = x; Y = y; Label = label } -> sprintf "%s: %A,%A" label x y
        | Circle(center = center, radius = radius) -> sprintf "%s r=%A" center.Label radius
        | Tagged(label, payload) -> sprintf "%s=%A" label payload
    let inspectCollection value =
        match value with
        | [] -> "empty list"
        | [ single ] -> sprintf "one: %A" single
        | (head :: _ as all) -> sprintf "head=%A count=%d" head all.Length
    let inspectPair = function
        | struct (0, _) -> None
        | struct (left, right) when left = right -> Some "equal"
        | struct _ -> Some "different"
    let numbers = [ 1; 2; 3; 4; 5 ]
    let squares = numbers |> List.map (fun n -> n * n)
    let lookup = Map [ "λ", λ; "answer", 42 ]
    let array = [| for n in 0 .. 4 -> n, n * n |]
    let choice = if buildMode = "debug" then Some λ else None
    let generated =
        seq {
            yield 0
            for number in 1 .. 3 do
                yield number
            yield! [ 8; 13; 21 ]
        }
    let asynchronous =
        async {
            let! answer = async { return 40 + 2 }
            do! Async.Sleep 1
            return sprintf "%s:%d" buildMode answer
        }
    let backgroundTask =
        task {
            let! result = System.Threading.Tasks.Task.FromResult λ
            return result |+| 1
        }
    let readFirstLine path =
        use reader = IO.File.OpenText path
        reader.ReadLine()
    let withCleanup action =
        try
            action ()
        finally
            printfn "cleanup // not a comment"
    let safelyParse text =
        try
            describeNumber text |> Ok
        with
        | ParseFailure(input, position) -> Error(sprintf "%s@%d" input position)
        | :? ArgumentException as error -> Error error.Message
    let mutable total = 0
    for number in numbers do
        total <- total + number
    while total < 20 do
        total <- total + 1
    let quotedAddition: Expr<int> = <@ λ + 1 @>
    let rawQuotation: Expr = <@@ 40 + 2 @@>
    let describer =
        { new IDescribe with
            member _.Describe prefix = prefix + 東京 + Rocket }
    [<Obsolete("Use describeNumber")>]
    let oldDescribe value = describeNumber value
    module Nested =
        let private secret = lazy (Guid.NewGuid())
        let reveal () = secret.Value

    // Operators that resemble punctuation: |> >> << <> <= >= && ||.
    let operatorPipeline =
        numbers
        |> List.filter (fun n -> n >= 2 && n <> 4)
        |> List.map ((+) 1)
        |> List.fold (+) 0
    (*
       A final multiline comment contains (* a closed nested comment *),
       Markdown-like `code`, λ, 東京, 🚀, and 𝌆.
    *)
