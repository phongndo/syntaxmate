# 1 "mission_telemetry.ml"

[@@@warning "-32"]

(** Mission telemetry simulator for the café ground station.
    It exercises ordinary OCaml forms rather than generated snippets. *)
(* A nested note remains one comment:
   (* packets may contain λ, 東京, and an uplink emoji 🛰️ *)
   every delimiter in this fixture is deliberately balanced. *)

type severity =
  | Debug
  | Info
  | Warning of string
  | Critical of { code : int; recoverable : bool }
type coordinate = {
  latitude : float;
  longitude : float;
  mutable altitude_m : int;
}
type status =
  | Nominal
  | Degraded of string list
  | Offline
and packet = {
  sequence : int64;
  source : string;
  position : coordinate option;
  status : status;
  tags : (string * string) list;
}
type 'a sample =
  { captured_at : float; value : 'a }
  [@@deriving show]
type _ field =
  | Callsign : string field
  | Battery : float field
  | Last_packet : packet option field
type event = ..
type event += Packet of packet | Alarm of severity
type response = [ `Accepted of int64 | `Retry_after of float | `Rejected ]
type ('key, 'value) cache = ('key * 'value) list
exception Link_down of string
exception Invalid_packet of int64 * string
external monotonic_now : unit -> float = "caml_sys_time_include_children"

module Mission_clock = struct
  let now () = Sys.time ()
end
module Default_config = struct
  let callsign = "Aster-7"
  let retry_limit = 0x3
  type transport = Radio | Optical
  let transport = Optical
end
module List_buffer = struct
  type 'a t = 'a list
  let empty = []
  let push item items = item :: items
  let pop = function
    | [] -> None
    | head :: tail -> Some (head, tail)
end
module Shared_buffer = List_buffer

module Monitor = struct
  let seen = ref 0
  let history = Array.make 8 None
  let remember packet =
    let slot = !seen mod Array.length history in
    history.(slot) <- Some { captured_at = Mission_clock.now (); value = packet };
    incr seen
  let classify { status; position; _ } =
    match status, position with
    | Offline, _ -> `Rejected
    | Degraded reasons, _ when List.length reasons > Default_config.retry_limit ->
        `Retry_after 2.5
    | (Nominal | Degraded _), Some { altitude_m; _ } when altitude_m < 0 ->
        `Rejected
    | _ -> `Accepted (Int64.of_int !seen)
end
module B = List_buffer

open Printf
let ( let* ) option f =
  match option with
  | Some value -> f value
  | None -> None
let ( ++ ) left right = left ^ " · " ^ right
let clamp ~low ~high value = max low (min high value)

let format_position ?(units = `Metric) ~prefix coordinate =
  let altitude =
    match units with
    | `Metric -> float coordinate.altitude_m
    | `Imperial -> float coordinate.altitude_m *. 3.28084
  in
  sprintf "%s %.4f, %.4f at %.1f" prefix
    coordinate.latitude coordinate.longitude altitude
let find_tag ~name packet = List.assoc_opt name packet.tags
let packet_name packet =
  let* mission = find_tag ~name:"mission" packet in
  Some (packet.source ++ mission)

let describe_severity = function
  | Debug -> "debug"
  | Info -> "information"
  | Warning message -> "warning: " ^ message
  | Critical { code = (0 | 1); recoverable = true } ->
      sprintf "recoverable critical %d" code
  | Critical { code; recoverable } ->
      sprintf "critical %04d (recoverable=%b)" code recoverable
let decode_field : type a. a field -> packet -> a =
  fun field packet ->
    match field with
    | Callsign -> packet.source
    | Battery ->
        Option.value ~default:nan
          (find_tag ~name:"battery" packet |> Option.map float_of_string)
    | Last_packet -> Some packet
let parse_altitude text =
  try int_of_string text with
  | Failure _ -> raise (Invalid_packet (Int64.minus_one, text))
  | Sys_error message as error ->
      eprintf "system error: %s\n%!" message;
      raise error
let update_position ({ altitude_m; _ } as coordinate) delta =
  coordinate.altitude_m <- clamp ~low:(-500) ~high:100_000 (altitude_m + delta)
let make_packet ?position ?(status = Nominal) ~sequence ~source tags =
  { sequence; source; position; status; tags }
let force_sample (lazy sample) = sample

let unpack_triplet = function
  | [| first; second; third |] -> Some (first, second, third)
  | [| |] -> None
  | _ -> invalid_arg "expected exactly three readings"
let summarize packet =
  match packet with
  | { source = ("Aster-7" | "Relay-2") as source;
      position = Some ({ altitude_m; _ } as coordinate); _ }
    when altitude_m >= 1_000 ->
      format_position ~prefix:source coordinate
  | { status = Degraded (reason :: _); _ } -> "degraded: " ^ reason
  | { status = Degraded []; _ } -> "degraded"
  | { status = Offline; source; _ } -> source ^ " is offline"
  | { position = None; _ } -> "no position"
  | _ -> "low altitude"

class virtual reporter name = object (self)
  val mutable delivered = 0
  method name = name
  method count = delivered
  method private mark = delivered <- delivered + 1
  method virtual emit : event -> unit
  method summary = sprintf "%s delivered %d events" self#name self#count
  initializer if name = "" then invalid_arg "empty reporter name"
end
class console_reporter ?(channel = stdout) name = object
  inherit reporter name as super
  method emit event =
    super#mark;
    match event with
    | Packet packet -> fprintf channel "packet: %s\n%!" (summarize packet)
    | Alarm severity -> fprintf channel "alarm: %s\n%!" (describe_severity severity)
    | _ -> fprintf channel "unknown extension event\n%!"
end
let quiet_reporter = object
  val mutable messages = ([] : string list)
  method emit text = messages <- text :: messages
  method messages = List.rev messages
end

let banner =
  {mission|
Mission Control — café shift
Uplink: 東京 / λ-sector / 🛰️
Operators may type "abort", but this quoted string is raw.
|mission}
let escaped = "line one\nline two\t\x41\o101\\\""
let character_samples = [ 'Z'; '\n'; '\065'; '\x42'; '\o103' ]
let numeric_samples =
  [ 42.; 1_000.25; 6.022e23; 0x1.fp3; float_of_int 0b101010; float_of_int 0o52 ]
let extension_example value = [%identity (value : int)]

let run_simulation () =
  let origin = { latitude = 48.8566; longitude = 2.3522; altitude_m = 35 } in
  let packet =
    make_packet ~position:origin ~sequence:0x2aL ~source:Default_config.callsign
      [ "mission", "Étoile"; "battery", "87.5" ]
  in
  let reporter = new console_reporter "primary" in
  let queue = B.empty |> B.push (Packet packet) |> B.push (Alarm Info) in
  let queue_ref = ref queue in
  let attempts = ref 0 in
  while !attempts < Default_config.retry_limit do
    incr attempts;
    update_position origin 125
  done;
  for index = 0 to Array.length numeric_samples - 1 do
    if index land 1 = 0 then quiet_reporter#emit (string_of_float numeric_samples.(index))
  done;
  for countdown = 2 downto 0 do
    printf "T-%d\n%!" countdown
  done;
  begin
    match B.pop !queue_ref with
    | Some (event, rest) ->
        queue_ref := rest;
        reporter#emit event
    | None -> raise (Link_down "empty telemetry queue")
  end;
  Monitor.remember packet;
  assert (reporter#count = 1);
  (reporter#summary, packet_name packet)

let () =
  print_endline banner;
  match run_simulation () with
  | summary, Some name -> printf "%s from %s\n" summary name
  | summary, None -> print_endline summary
  | exception Link_down reason -> eprintf "link down: %s\n" reason
