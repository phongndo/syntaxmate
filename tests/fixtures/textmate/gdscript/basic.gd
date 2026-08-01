@tool
class_name UnicodeGreeter
extends Node

signal greeted(message: String, count: int)

const MAX_GREETINGS := 3
@export_multiline var greeting: String = "Olá, 世界 🚀"
@export_range(0.1, 2.0, 0.1) var delay_seconds := 0.5
@onready var label: Label = $Panel/GreetingLabel
var history: Array[String] = []

func _ready() -> void:
	label.text = greeting
	for index in range(MAX_GREETINGS):
		if index % 2 == 0 and not greeting.is_empty():
			history.append("%02d: %s" % [index, greeting])
		else:
			continue
	greeted.emit("Queued {count} greetings".format({"count": history.size()}), history.size())

func format_name(name: String = "訪問者") -> String:
	var decorated := "✨ %s ✨" % name.strip_edges()
	return decorated if name.length() > 0 else "anonymous"
