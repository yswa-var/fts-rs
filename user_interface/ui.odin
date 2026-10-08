package main

import rl "vendor:raylib"

EDITOR_TOOLBAR :: rl.Rectangle{x = 0, y = 0, width = 1280, height = 58}
SPAWN_BUTTON   :: rl.Rectangle{x = 16, y = 11, width = 140, height = 36}
RUN_BUTTON     :: rl.Rectangle{x = 166, y = 11, width = 80, height = 36}

point_is_in_toolbar :: proc(point: rl.Vector2) -> bool {
	toolbar := EDITOR_TOOLBAR
	toolbar.width = f32(rl.GetScreenWidth())
	return rl.CheckCollisionPointRec(point, toolbar)
}

spawn_button_was_clicked :: proc() -> bool {
	return rl.IsMouseButtonPressed(.LEFT) &&
		rl.CheckCollisionPointRec(rl.GetMousePosition(), SPAWN_BUTTON)
}

run_button_was_clicked :: proc() -> bool {
	return rl.IsMouseButtonPressed(.LEFT) &&
		rl.CheckCollisionPointRec(rl.GetMousePosition(), RUN_BUTTON)
}

draw_editor_toolbar :: proc(state: ^App_State) {
	toolbar := EDITOR_TOOLBAR
	toolbar.width = f32(rl.GetScreenWidth())
	rl.DrawRectangleRec(toolbar, rl.Color{32, 38, 50, 245})

	button_color := rl.DARKBLUE
	if rl.CheckCollisionPointRec(rl.GetMousePosition(), SPAWN_BUTTON) {
		button_color = rl.BLUE
	}
	rl.DrawRectangleRec(SPAWN_BUTTON, button_color)
	rl.DrawText("Spawn RSI", 42, 21, 18, rl.WHITE)

	run_color := rl.DARKGREEN
	if rl.CheckCollisionPointRec(rl.GetMousePosition(), RUN_BUTTON) {
		run_color = rl.GREEN
	}
	rl.DrawRectangleRec(RUN_BUTTON, run_color)
	rl.DrawText("Run", 190, 21, 18, rl.WHITE)

	rl.DrawText("Node machinery", 265, 20, 18, rl.WHITE)
	rl.DrawText("Wheel: zoom", 415, 20, 18, rl.LIGHTGRAY)
	rl.DrawText("Middle drag: pan", 545, 20, 18, rl.LIGHTGRAY)
	rl.DrawText("Drag output → input", 715, 20, 18, rl.LIGHTGRAY)
	rl.DrawText("Space: RSI", 900, 20, 18, rl.LIGHTGRAY)

	selected_label: cstring = "Selected: none"
	if state.selected >= 0 {
		selected_label = rl.TextFormat("Selected: node %i", state.nodes[state.selected].id)
	}
	rl.DrawText(selected_label, 1010, 20, 18, rl.WHITE)
	rl.DrawText(state.last_status, 16, 76, 18, rl.DARKGRAY)
}
