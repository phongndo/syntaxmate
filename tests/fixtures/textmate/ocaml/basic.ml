(* Variants, records, and Unicode strings: café 🚀 𝌆. *)
type mood =
  | Calm
  | Busy of int

let describe ?(prefix = "status") mood =
  match mood with
  | Calm -> Printf.sprintf "%s: resting" prefix
  | Busy count when count > 0 ->
      Printf.sprintf "%s: %d jobs 🚀" prefix count
  | Busy _ -> "all done"

module Counter = struct
  type t = { mutable value : int }
  let create value = { value }
  let bump counter = counter.value <- counter.value + 1
end

let () =
  let counter = Counter.create 0x29 in
  Counter.bump counter;
  [ Calm; Busy counter.value ]
  |> List.map (describe ~prefix:"λ café")
  |> List.iter print_endline
