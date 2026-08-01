#light "on"
module BasicFixture

open System

/// **Basic** F# fixture with λ, 東京, 🚀, and 𝌆.
[<Measure>]
type kg

[<Struct>]
type Point = { X: float<kg>; Y: float<kg> }

type Choice<'T> = Ok of 'T | Error of message: string

let λ = 0x2A
let 東京 = { X = 1.5<kg>; Y = 2.5<kg> }
let (|Even|Odd|) n = if n % 2 = 0 then Even else Odd

(* A nested (* block *) comment containing // and 🚀. *)
let describe value =
    match value with
    | Even -> sprintf "even: %04d" value
    | Odd -> @"odd: 東京"

let quoted = <@ describe λ @>
printfn "%s — %A — 𝌆" (describe λ) 東京 quoted
