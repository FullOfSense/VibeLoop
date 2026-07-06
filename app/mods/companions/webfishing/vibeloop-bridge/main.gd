# VibeLoop bridge for WEBFISHING (GDWeave mod).
# Watches the local player and appends JSON lines to user://vibeloop.jsonl:
#   {"e":"bite"}         a fish is on the line (FISHING_STRUGGLE)
#   {"e":"catch","n":1}  fish landed (PlayerData.fish_caught increased)
#   {"e":"levelup","n"}  rod level went up
# VibeLoop tails that file. Read-only observer: changes no gameplay.
extends Node

const OUT := "user://vibeloop.jsonl"

var player = null
var last_state := -1
var last_fish := -1
var last_level := -1

func _ready() -> void:
	var f := File.new()
	f.open(OUT, File.WRITE) # fresh file per game launch
	f.close()
	PlayerData.connect("_xp_add", self, "_on_xp_add")

func _emit(line: String) -> void:
	var f := File.new()
	if f.open(OUT, File.READ_WRITE) == OK:
		f.seek_end()
		f.store_line(line)
		f.close()

func _on_xp_add(_amt, lvl, _total) -> void:
	if last_level != -1 and lvl > last_level:
		_emit('{"e":"levelup","n":%d}' % lvl)
	last_level = lvl

func _process(_delta: float) -> void:
	if player == null or not is_instance_valid(player):
		var controlled := get_tree().get_nodes_in_group("controlled_player")
		if controlled.size() > 0:
			player = controlled[0]
			last_state = -1
		return

	var state: int = player.state
	if state != last_state:
		if state == player.STATES.FISHING_STRUGGLE:
			_emit('{"e":"bite"}')
		last_state = state

	var fc: int = PlayerData.fish_caught
	if last_fish == -1:
		last_fish = fc
	elif fc > last_fish:
		_emit('{"e":"catch","n":%d}' % (fc - last_fish))
		last_fish = fc
