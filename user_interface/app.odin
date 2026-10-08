package main

import rl "vendor:raylib"
import "core:math"

INITIAL_WINDOW_WIDTH  :: 1280
INITIAL_WINDOW_HEIGHT :: 720
WINDOW_TITLE          :: "ROCKET - Visual Quant Engine"
GRID_SIZE             :: 50.0
MIN_CAMERA_ZOOM       :: 0.25
MAX_CAMERA_ZOOM       :: 4.0

Node_Kind :: enum {
	CONSTANT,
	ADD,
	PROBE,
	RSI,
	MARKET_BARS,
	CLOSE,
	SMA,
}

Port_Direction :: enum {
	INPUT,
	OUTPUT,
}

Port_Value_Type :: enum {
	FLOAT,
	INTEGER,
	SERIES,
	BARS,
}

Node_Port_Definition :: struct {
	name:       cstring,
	direction:  Port_Direction,
	value_type: Port_Value_Type,
}

Node_Definition :: struct {
	kind:         Node_Kind,
	title:        cstring,
	width:        f32,
	height:       f32,
	inputs:       [4]Node_Port_Definition,
	input_count:  int,
	outputs:      [2]Node_Port_Definition,
	output_count: int,
}

Node_Instance :: struct {
	id:         int,
	definition: Node_Definition,
	position:   rl.Vector2,
	number:     f32,
	result:     f32,
	evaluated:  bool,
}

Graph_Edge :: struct {
	from_node_id: int,
	from_port:    int,
	to_node_id:   int,
	to_port:      int,
	value_type:   Port_Value_Type,
}

Port_Hit :: struct {
	valid:       bool,
	node_index:  int,
	port_index:  int,
	direction:   Port_Direction,
}

App_State :: struct {
	camera:              rl.Camera2D,
	nodes:               [dynamic]Node_Instance,
	edges:               [dynamic]Graph_Edge,
	selected:            int,
	dragging:            bool,
	drag_offset:         rl.Vector2,
	next_node_id:        int,
	connecting:          bool,
	connecting_node:     int,
	connecting_port:     int,
	last_status:         cstring,
	last_probe_value:    f32,
	graph_ran:           bool,
}

run_main_window :: proc() {
	rl.SetConfigFlags(rl.ConfigFlags{.WINDOW_RESIZABLE, .VSYNC_HINT})
	rl.InitWindow(INITIAL_WINDOW_WIDTH, INITIAL_WINDOW_HEIGHT, WINDOW_TITLE)
	defer rl.CloseWindow()
	rl.SetTargetFPS(60)

	state := App_State{
		camera = rl.Camera2D{
			offset = rl.Vector2{f32(INITIAL_WINDOW_WIDTH / 2), f32(INITIAL_WINDOW_HEIGHT / 2)},
			target = rl.Vector2{0, 0},
			zoom = 1.0,
		},
		selected = -1,
		connecting_node = -1,
		last_status = "Connect output ports to input ports",
		next_node_id = 1,
	}

	constant_a := spawn_node(&state, .CONSTANT, rl.Vector2{-410, -80})
	state.nodes[constant_a].number = 10
	constant_b := spawn_node(&state, .CONSTANT, rl.Vector2{-410, 100})
	state.nodes[constant_b].number = 20
	add_node := spawn_node(&state, .ADD, rl.Vector2{-100, 0})
	probe_node := spawn_node(&state, .PROBE, rl.Vector2{230, 0})

	connect_edge(&state, constant_a, 0, add_node, 0)
	connect_edge(&state, constant_b, 0, add_node, 1)
	connect_edge(&state, add_node, 0, probe_node, 0)
	evaluate_graph(&state)

	for !rl.WindowShouldClose() {
		update_app(&state)
		draw_app(&state)
	}
}

node_definition :: proc(kind: Node_Kind) -> Node_Definition {
	definition := Node_Definition{
		kind = kind,
		width = 220,
		height = 110,
	}

	switch kind {
	case .CONSTANT:
		definition.title = "Constant"
		definition.outputs[0] = Node_Port_Definition{name = "value", direction = .OUTPUT, value_type = .FLOAT}
		definition.output_count = 1
	case .ADD:
		definition.title = "Add"
		definition.inputs[0] = Node_Port_Definition{name = "a", direction = .INPUT, value_type = .FLOAT}
		definition.inputs[1] = Node_Port_Definition{name = "b", direction = .INPUT, value_type = .FLOAT}
		definition.input_count = 2
		definition.outputs[0] = Node_Port_Definition{name = "sum", direction = .OUTPUT, value_type = .FLOAT}
		definition.output_count = 1
	case .PROBE:
		definition.title = "Probe"
		definition.inputs[0] = Node_Port_Definition{name = "value", direction = .INPUT, value_type = .FLOAT}
		definition.input_count = 1
	case .RSI:
		definition.title = "RSI"
		definition.inputs[0] = Node_Port_Definition{name = "series", direction = .INPUT, value_type = .SERIES}
		definition.inputs[1] = Node_Port_Definition{name = "period", direction = .INPUT, value_type = .INTEGER}
		definition.input_count = 2
		definition.outputs[0] = Node_Port_Definition{name = "rsi", direction = .OUTPUT, value_type = .SERIES}
		definition.output_count = 1
	case .MARKET_BARS:
		definition.title = "Market Bars"
		definition.outputs[0] = Node_Port_Definition{name = "bars", direction = .OUTPUT, value_type = .BARS}
		definition.output_count = 1
	case .CLOSE:
		definition.title = "Close"
		definition.inputs[0] = Node_Port_Definition{name = "bars", direction = .INPUT, value_type = .BARS}
		definition.input_count = 1
		definition.outputs[0] = Node_Port_Definition{name = "close", direction = .OUTPUT, value_type = .SERIES}
		definition.output_count = 1
	case .SMA:
		definition.title = "SMA"
		definition.inputs[0] = Node_Port_Definition{name = "series", direction = .INPUT, value_type = .SERIES}
		definition.inputs[1] = Node_Port_Definition{name = "period", direction = .INPUT, value_type = .INTEGER}
		definition.input_count = 2
		definition.outputs[0] = Node_Port_Definition{name = "sma", direction = .OUTPUT, value_type = .SERIES}
		definition.output_count = 1
	}

	max_ports := definition.input_count
	if definition.output_count > max_ports {
		max_ports = definition.output_count
	}
	definition.height = 48 + f32(max_ports * 30)
	return definition
}

spawn_node :: proc(state: ^App_State, kind: Node_Kind, position: rl.Vector2) -> int {
	node := Node_Instance{
		id = state.next_node_id,
		definition = node_definition(kind),
		position = position,
		number = 14,
	}
	state.next_node_id += 1
	append(&state.nodes, node)
	state.selected = len(state.nodes) - 1
	return state.selected
}

node_bounds :: proc(node: Node_Instance) -> rl.Rectangle {
	return rl.Rectangle{
		x = node.position.x,
		y = node.position.y,
		width = node.definition.width,
		height = node.definition.height,
	}
}

update_app :: proc(state: ^App_State) {
	width := rl.GetScreenWidth()
	height := rl.GetScreenHeight()
	state.camera.offset = rl.Vector2{f32(width) / 2, f32(height) / 2}

	if spawn_button_was_clicked() || rl.IsKeyPressed(.SPACE) {
		mouse_world := rl.GetScreenToWorld2D(rl.Vector2{f32(width) / 2, f32(height) / 2}, state.camera)
		spawn_node(state, .RSI, mouse_world)
		state.last_status = "Spawned RSI node from its definition"
		return
	}
	if run_button_was_clicked() || rl.IsKeyPressed(.R) {
		evaluate_graph(state)
		return
	}

	mouse_screen := rl.GetMousePosition()
	if !point_is_in_toolbar(mouse_screen) {
		update_camera(state, mouse_screen)
		update_node_selection(state, mouse_screen)
	}
	if rl.IsMouseButtonPressed(.RIGHT) {
		state.connecting = false
		state.last_status = "Connection cancelled"
	}
}

update_camera :: proc(state: ^App_State, mouse_screen: rl.Vector2) {
	if rl.IsMouseButtonDown(.MIDDLE) {
		delta := rl.GetMouseDelta()
		state.camera.target.x -= delta.x / state.camera.zoom
		state.camera.target.y -= delta.y / state.camera.zoom
	}
	wheel := rl.GetMouseWheelMove()
	if wheel != 0 {
		before := rl.GetScreenToWorld2D(mouse_screen, state.camera)
		state.camera.zoom = math.clamp(state.camera.zoom * (1.0 + wheel * 0.1), MIN_CAMERA_ZOOM, MAX_CAMERA_ZOOM)
		after := rl.GetScreenToWorld2D(mouse_screen, state.camera)
		state.camera.target.x += before.x - after.x
		state.camera.target.y += before.y - after.y
	}
}

update_node_selection :: proc(state: ^App_State, mouse_screen: rl.Vector2) {
	mouse_world := rl.GetScreenToWorld2D(mouse_screen, state.camera)
	if rl.IsMouseButtonPressed(.LEFT) {
		hit := find_port_at(state.nodes[:], mouse_world, state.camera.zoom)
		if state.connecting {
			if hit.valid && hit.direction == .INPUT {
				connect_edge(state, state.connecting_node, state.connecting_port, hit.node_index, hit.port_index)
				state.connecting = false
				return
			}
			state.last_status = "Choose a compatible input port"
			return
		}
		if hit.valid && hit.direction == .OUTPUT {
			state.connecting = true
			state.connecting_node = hit.node_index
			state.connecting_port = hit.port_index
			state.selected = hit.node_index
			state.last_status = "Choose an input port"
			return
		}

		state.selected = find_node_at(state.nodes[:], mouse_world)
		state.dragging = state.selected >= 0
		if state.dragging {
			node := &state.nodes[state.selected]
			state.drag_offset = rl.Vector2{mouse_world.x - node.position.x, mouse_world.y - node.position.y}
		}
	}
	if state.dragging && rl.IsMouseButtonDown(.LEFT) && state.selected >= 0 {
		node := &state.nodes[state.selected]
		node.position.x = mouse_world.x - state.drag_offset.x
		node.position.y = mouse_world.y - state.drag_offset.y
	}
	if rl.IsMouseButtonReleased(.LEFT) {
		state.dragging = false
	}
}

find_node_at :: proc(nodes: []Node_Instance, point: rl.Vector2) -> int {
	for i := len(nodes) - 1; i >= 0; i -= 1 {
		if rl.CheckCollisionPointRec(point, node_bounds(nodes[i])) {
			return i
		}
	}
	return -1
}

find_port_at :: proc(nodes: []Node_Instance, point: rl.Vector2, zoom: f32) -> Port_Hit {
	hit := Port_Hit{valid = false, node_index = -1, port_index = -1}
	radius := 12 / zoom
	for node_index := len(nodes) - 1; node_index >= 0; node_index -= 1 {
		node := nodes[node_index]
		for port_index := 0; port_index < node.definition.input_count; port_index += 1 {
			position := node_port_position(node, .INPUT, port_index)
			port_bounds := rl.Rectangle{x = position.x - radius, y = position.y - radius, width = radius * 2, height = radius * 2}
			if rl.CheckCollisionPointRec(point, port_bounds) {
				return Port_Hit{valid = true, node_index = node_index, port_index = port_index, direction = .INPUT}
			}
		}
		for port_index := 0; port_index < node.definition.output_count; port_index += 1 {
			position := node_port_position(node, .OUTPUT, port_index)
			port_bounds := rl.Rectangle{x = position.x - radius, y = position.y - radius, width = radius * 2, height = radius * 2}
			if rl.CheckCollisionPointRec(point, port_bounds) {
				return Port_Hit{valid = true, node_index = node_index, port_index = port_index, direction = .OUTPUT}
			}
		}
	}
	return hit
}

connect_edge :: proc(state: ^App_State, from_index, from_port, to_index, to_port: int) {
	if from_index == to_index {
		state.last_status = "A node cannot connect to itself"
		return
	}
	source := state.nodes[from_index]
	target := state.nodes[to_index]
	source_port := source.definition.outputs[from_port]
	target_port := target.definition.inputs[to_port]
	if source_port.value_type != target_port.value_type {
		state.last_status = "Rejected: port types do not match"
		return
	}

	for i := len(state.edges) - 1; i >= 0; i -= 1 {
		if state.edges[i].to_node_id == target.id && state.edges[i].to_port == to_port {
			ordered_remove_edge(&state.edges, i)
		}
	}
	append(&state.edges, Graph_Edge{
		from_node_id = source.id,
		from_port = from_port,
		to_node_id = target.id,
		to_port = to_port,
		value_type = source_port.value_type,
	})
	state.last_status = "Connected typed ports"
}

ordered_remove_edge :: proc(edges: ^[dynamic]Graph_Edge, index: int) {
	for i := index; i < len(edges^)-1; i += 1 {
		edges^[i] = edges^[i + 1]
	}
	pop(edges)
}

node_index_by_id :: proc(nodes: []Node_Instance, id: int) -> int {
	for node, i in nodes {
		if node.id == id {
			return i
		}
	}
	return -1
}

input_value :: proc(state: ^App_State, node_id: int, input_port: int) -> (f32, bool) {
	for edge in state.edges {
		if edge.to_node_id == node_id && edge.to_port == input_port {
			source_index := node_index_by_id(state.nodes[:], edge.from_node_id)
			if source_index >= 0 {
				return state.nodes[source_index].result, state.nodes[source_index].evaluated
			}
		}
	}
	return 0, false
}

topological_order :: proc(state: ^App_State) -> ([dynamic]int, bool) {
	indegree := make([]int, len(state.nodes))
	queue: [dynamic]int
	order: [dynamic]int
	for edge in state.edges {
		target := node_index_by_id(state.nodes[:], edge.to_node_id)
		if target >= 0 {
			indegree[target] += 1
		}
	}
	for degree, i in indegree {
		if degree == 0 {
			append(&queue, i)
		}
	}
	for head := 0; head < len(queue); head += 1 {
		current := queue[head]
		append(&order, current)
		for edge in state.edges {
			if edge.from_node_id == state.nodes[current].id {
				target := node_index_by_id(state.nodes[:], edge.to_node_id)
				indegree[target] -= 1
				if indegree[target] == 0 {
					append(&queue, target)
				}
			}
		}
	}
	return order, len(order) == len(state.nodes)
}

evaluate_graph :: proc(state: ^App_State) {
	order, valid := topological_order(state)
	if !valid {
		state.graph_ran = false
		state.last_status = "Run failed: graph contains a cycle"
		return
	}
	for i := 0; i < len(state.nodes); i += 1 {
		node := &state.nodes[i]
		node.evaluated = false
		node.result = 0
	}
	for index in order {
		node := &state.nodes[index]
		switch node.definition.kind {
		case .CONSTANT:
			node.result = node.number
			node.evaluated = true
		case .ADD:
			a, a_ok := input_value(state, node.id, 0)
			b, b_ok := input_value(state, node.id, 1)
			if a_ok && b_ok {
				node.result = a + b
				node.evaluated = true
			}
		case .PROBE:
			value, ok := input_value(state, node.id, 0)
			if ok {
				node.result = value
				state.last_probe_value = value
				node.evaluated = true
			}
		case .RSI, .MARKET_BARS, .CLOSE, .SMA:
			// These definitions are available for typed graph construction.
			// Their domain evaluators arrive with the series engine milestone.
		}
	}
	state.graph_ran = true
	state.last_status = rl.TextFormat("Graph executed: OUTPUT = %.0f", state.last_probe_value)
}

draw_app :: proc(state: ^App_State) {
	rl.BeginDrawing()
	defer rl.EndDrawing()
	rl.ClearBackground(rl.RAYWHITE)
	rl.BeginMode2D(state.camera)
	draw_grid(state.camera)
	draw_edges(state)
	for node, i in state.nodes {
		draw_node(node, i == state.selected, state.camera.zoom)
	}
	if state.connecting && state.connecting_node >= 0 {
		source := node_port_position(state.nodes[state.connecting_node], .OUTPUT, state.connecting_port)
		mouse := rl.GetScreenToWorld2D(rl.GetMousePosition(), state.camera)
		rl.DrawLineEx(source, mouse, 4 / state.camera.zoom, rl.ORANGE)
	}
	rl.EndMode2D()
	draw_editor_toolbar(state)
	rl.DrawFPS(16, rl.GetScreenHeight() - 28)
}

draw_edges :: proc(state: ^App_State) {
	for edge in state.edges {
		from_index := node_index_by_id(state.nodes[:], edge.from_node_id)
		to_index := node_index_by_id(state.nodes[:], edge.to_node_id)
		if from_index < 0 || to_index < 0 {
			continue
		}
		from := node_port_position(state.nodes[from_index], .OUTPUT, edge.from_port)
		to := node_port_position(state.nodes[to_index], .INPUT, edge.to_port)
		rl.DrawLineEx(from, to, 5 / state.camera.zoom, port_type_color(edge.value_type))
	}
}

draw_node :: proc(node: Node_Instance, selected: bool, zoom: f32) {
	bounds := node_bounds(node)
	rl.DrawRectangleRec(bounds, rl.Color{238, 242, 248, 255})
	rl.DrawRectangleRec(
		rl.Rectangle{x = bounds.x, y = bounds.y, width = bounds.width, height = 30},
		rl.Color{39, 76, 119, 255},
	)
	rl.DrawText(node.definition.title, i32(bounds.x + 12), i32(bounds.y + 7), 18, rl.WHITE)
	if node.definition.kind == .CONSTANT {
		rl.DrawText(rl.TextFormat("%.0f", node.number), i32(bounds.x + 12), i32(bounds.y + 70), 22, rl.DARKGRAY)
	}
	if node.definition.kind == .PROBE && node.evaluated {
		rl.DrawText(rl.TextFormat("OUTPUT = %.0f", node.result), i32(bounds.x + 12), i32(bounds.y + 70), 18, rl.DARKGREEN)
	}
	for i := 0; i < node.definition.input_count; i += 1 {
		port := node.definition.inputs[i]
		position := node_port_position(node, .INPUT, i)
		rl.DrawCircleV(position, 7 / zoom, port_type_color(port.value_type))
		rl.DrawText(port.name, i32(position.x + 12), i32(position.y - 8), 16, rl.DARKGRAY)
	}
	for i := 0; i < node.definition.output_count; i += 1 {
		port := node.definition.outputs[i]
		position := node_port_position(node, .OUTPUT, i)
		label_width := rl.MeasureText(port.name, 16)
		rl.DrawCircleV(position, 7 / zoom, port_type_color(port.value_type))
		rl.DrawText(port.name, i32(position.x) - label_width - 12, i32(position.y - 8), 16, rl.DARKGRAY)
	}
	if selected {
		rl.DrawRectangleLinesEx(bounds, 3 / zoom, rl.ORANGE)
	}
}

node_port_position :: proc(node: Node_Instance, direction: Port_Direction, index: int) -> rl.Vector2 {
	y := node.position.y + 54 + f32(index * 30)
	if direction == .OUTPUT {
		return rl.Vector2{node.position.x + node.definition.width, y}
	}
	return rl.Vector2{node.position.x, y}
}

port_type_color :: proc(value_type: Port_Value_Type) -> rl.Color {
	switch value_type {
	case .FLOAT:
		return rl.ORANGE
	case .INTEGER:
		return rl.GOLD
	case .SERIES:
		return rl.SKYBLUE
	case .BARS:
		return rl.VIOLET
	}
	return rl.GRAY
}

draw_grid :: proc(camera: rl.Camera2D) {
	width := rl.GetScreenWidth()
	height := rl.GetScreenHeight()
	min_world := rl.GetScreenToWorld2D(rl.Vector2{}, camera)
	max_world := rl.GetScreenToWorld2D(rl.Vector2{f32(width), f32(height)}, camera)
	start_x := math.floor(min_world.x / GRID_SIZE) * GRID_SIZE
	start_y := math.floor(min_world.y / GRID_SIZE) * GRID_SIZE
	grid_color := rl.Color{220, 224, 230, 255}
	for x := start_x; x <= max_world.x + GRID_SIZE; x += GRID_SIZE {
		rl.DrawLineV(rl.Vector2{x, min_world.y}, rl.Vector2{x, max_world.y}, grid_color)
	}
	for y := start_y; y <= max_world.y + GRID_SIZE; y += GRID_SIZE {
		rl.DrawLineV(rl.Vector2{min_world.x, y}, rl.Vector2{max_world.x, y}, grid_color)
	}
	axis_color := rl.Color{150, 160, 175, 255}
	rl.DrawLineV(rl.Vector2{0, min_world.y}, rl.Vector2{0, max_world.y}, axis_color)
	rl.DrawLineV(rl.Vector2{min_world.x, 0}, rl.Vector2{max_world.x, 0}, axis_color)
}
