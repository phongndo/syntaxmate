@tool
@icon("res://icons/orbit_controller.svg")
class_name OrbitController
extends Node2D

## A deliberately feature-rich Godot 4 controller for syntax highlighting.
## It pilots probes named “Aurora” and “星舟” through a tiny simulation. 🛰️

#region API
signal launched(probe_name: String, velocity: Vector2)
signal telemetry_received(packet: Dictionary)
signal mission_finished

enum FlightState {
	IDLE,
	COUNTDOWN,
	ASCENDING,
	ORBITING,
	RETURNING,
}

class TelemetryPacket:
	extends RefCounted

	var sequence: int
	var message: String
	var samples: PackedFloat32Array

	func _init(id: int, text: String, values := PackedFloat32Array()) -> void:
		sequence = id
		message = text
		samples = values

	func as_dictionary() -> Dictionary:
		return {"sequence": sequence, "message": message, "samples": samples}

	static func empty() -> TelemetryPacket:
		return TelemetryPacket.new(0, "empty")

const APP_NAME: StringName = &"OrbitLab"
const HOME_PATH: NodePath = ^"Stations/LaunchPad"
const BYTE_MASK := 0b1111_0000
const COLOR_KEY := 0xCA_FE_BA_BE
const LIGHT_SPEED := 299_792_458
const EPSILON := .000_001
const SCIENTIFIC := 6.022_140_76e23
const NEGATIVE_EXPONENT := 1.0e-9

@export_category("Mission")
@export var probe_name := "Aurora 🚀"
@export_enum("Low:120", "Medium:240", "High:480") var target_altitude := 240
@export_range(0.0, 100.0, 0.5, "suffix:%") var throttle := 72.5
@export_flags("Camera", "Radio", "Sampler", "Return") var payload_flags := 0b1011
@export_multiline var briefing := """Launch from São Paulo.
Track the 北極 beacon.
Return before the ☄️ shower."""

@export_group("Presentation", "ui_")
@export_color_no_alpha var ui_accent := Color("6ad5ff")
@export_file("*.json") var ui_mission_file := "res://missions/demo.json"
@export_dir var ui_log_directory := "user://logs"
@export_node_path("Label") var ui_status_label_path: NodePath

@export_subgroup("Audio", "audio_")
@export_range(-60.0, 0.0, 0.5, "suffix:dB") var audio_volume_db := -8.0
@export_storage var persisted_launches: int = 0

@onready var launch_pad: Marker2D = $Stations/LaunchPad
@onready var status_label: Label = %StatusLabel
@onready var camera: Camera2D = $"Rig/Main Camera"
@onready var optional_radio: Node = get_node_or_null("Payload/%Radio")

var state: FlightState = FlightState.IDLE
var velocity := Vector2.ZERO
var orbit_points: Array[Vector2] = []
var metadata: Dictionary = {
	"operator": "Zoë",
	"vehicle": "星舟",
	"emoji": "🛰️",
}
var _energy := 100.0

var energy: float:
	get:
		return _energy
	set(value):
		_energy = clampf(value, 0.0, 100.0)

var status_text: String:
	get:
		return "%s — %.1f%%" % [FlightState.keys()[state], energy]

func _ready() -> void:
	assert(launch_pad != null, "Launch pad is required")
	status_label.text = "Ready: {name}".format({"name": probe_name})
	telemetry_received.connect(_on_telemetry_received)
	var home := NodePath("Stations/LaunchPad")
	print("Home path: %s" % home)
	if optional_radio is Node:
		optional_radio.set("enabled", true)

func begin_launch(delay_seconds: float = 3.0) -> void:
	if state != FlightState.IDLE:
		push_warning("Launch ignored while %s" % FlightState.keys()[state])
		return

	state = FlightState.COUNTDOWN
	for second in range(ceili(delay_seconds), 0, -1):
		status_label.text = "T−%d…" % second
		await get_tree().create_timer(1.0).timeout
	launched.emit(probe_name, velocity)
	state = FlightState.ASCENDING
	persisted_launches += 1

func _physics_process(delta: float) -> void:
	match state:
		FlightState.IDLE:
			velocity = velocity.move_toward(Vector2.ZERO, delta * 10.0)
		FlightState.ASCENDING when energy > 0.0:
			velocity.y -= throttle * delta
			energy -= delta * 2.5
			if global_position.y < -target_altitude:
				state = FlightState.ORBITING
		FlightState.ORBITING:
			_apply_orbit(delta)
		FlightState.RETURNING:
			velocity = global_position.direction_to(launch_pad.global_position) * 90.0
		_:
			pass
	position += velocity * delta

func _apply_orbit(delta: float) -> void:
	var angle := Time.get_ticks_msec() / 1000.0
	var wobble := sin(angle * 2.0) * 0.25
	velocity = Vector2.from_angle(angle + wobble) * (throttle ** 0.5)
	orbit_points.append(global_position)
	while orbit_points.size() > 256:
		orbit_points.pop_front()
	if Input.is_action_just_pressed("ui_cancel"):
		state = FlightState.RETURNING
	elif energy <= EPSILON:
		finish_mission(false)

func summarize_samples(samples: PackedFloat32Array) -> Dictionary:
	var total := 0.0
	var minimum := INF
	var maximum := -INF
	for sample in samples:
		total += sample
		minimum = minf(minimum, sample)
		maximum = maxf(maximum, sample)
	var average := total / samples.size() if not samples.is_empty() else NAN
	return {"min": minimum, "max": maximum, "average": average}

func classify_code(code: int) -> String:
	match code:
		0, 1:
			return "nominal"
		2 when payload_flags & 0b0010:
			return "radio warning"
		var captured when captured == 3:
			return "captured %d" % captured
		_:
			return "unknown"

func exercise_operators(input: int) -> int:
	var bits := (input << 2) | BYTE_MASK
	bits ^= 0x0F
	bits &= 0xFF
	bits >>= 1
	bits |= 0b1
	var inverted := ~bits
	var arithmetic := ((input + 4) * 3 - 2) / 2
	arithmetic %= 7
	arithmetic **= 2
	var checks := input >= 0 && input <= 100 || input == -1
	if checks and input in [0, 1, 2] and not input != input:
		return arithmetic + (inverted ^ bits)
	return -1

func build_formatter(prefix: String) -> Callable:
	var suffix := "✓"
	var formatter := func(value: Variant) -> String:
		return "%s: %s %s" % [prefix, str(value), suffix]
	return formatter

func string_gallery() -> Array[String]:
	var escaped := "tabs\tquotes\"slash\\newline\n"
	var single := 'single-quoted λ'
	var raw_path := r"C:\missions\new\probe"
	var percent_format := "vector=%+08.2f, hex=%04X, text=%-10s"
	var brace_format := "{pilot:^12s} | {score:08.2f} | {{literal}}"
	var poem := '''First orbit: 🌍
Second orbit: 月
Third orbit: home.'''
	return [escaped, single, raw_path, percent_format, brace_format, poem]

func make_packet(sequence: int, message := "nominal") -> TelemetryPacket:
	var samples := PackedFloat32Array([1.25, 2.5, 5.0])
	return TelemetryPacket.new(sequence, message, samples)

@rpc("authority", "call_remote", "reliable", 0)
func submit_telemetry(packet_data: Dictionary) -> void:
	var packet := TelemetryPacket.new(
		packet_data.get("sequence", 0),
		packet_data.get("message", "missing"),
		PackedFloat32Array(packet_data.get("samples", [])),
	)
	telemetry_received.emit(packet.as_dictionary())

@warning_ignore("integer_division")
func coarse_progress(completed: int, total: int) -> int:
	return completed * 100 / maxi(total, 1)

func load_mission(path: String) -> Dictionary:
	if not FileAccess.file_exists(path):
		return {}
	var file := FileAccess.open(path, FileAccess.READ)
	var parsed: Variant = JSON.parse_string(file.get_as_text())
	return parsed as Dictionary if parsed is Dictionary else {}

func finish_mission(success: bool) -> void:
	state = FlightState.IDLE
	velocity = Vector2.ZERO
	status_label.text = "Mission complete ✅" if success else "Mission aborted ⚠️"
	mission_finished.emit()
#endregion
