package main

import rl "vendor:raylib"

INITIAL_WINDOW_WIDTH  :: 1280
INITIAL_WINDOW_HEIGHT :: 720
WINDOW_TITLE          :: "raylib + Odin starter"

App_State :: struct {
	accent: rl.Color,
	clicks: int,
}

run_main_window :: proc() {
	rl.SetConfigFlags(rl.ConfigFlags{.WINDOW_RESIZABLE, .VSYNC_HINT})
	rl.InitWindow(INITIAL_WINDOW_WIDTH, INITIAL_WINDOW_HEIGHT, WINDOW_TITLE)
	defer rl.CloseWindow()
	rl.SetTargetFPS(60)

	state := App_State{accent = rl.SKYBLUE}
	for !rl.WindowShouldClose() {
		update_app(&state)
		draw_app(&state)
	}
}

update_app :: proc(state: ^App_State) {
	if main_button_was_clicked() {
		state.clicks += 1
		launch_second_window()
	}
}

draw_app :: proc(state: ^App_State) {
	rl.BeginDrawing()
	defer rl.EndDrawing()

	rl.ClearBackground(rl.RAYWHITE)
	width := rl.GetScreenWidth()
	height := rl.GetScreenHeight()

	rl.DrawText("Odin + raylib", 40, 36, 32, rl.DARKGRAY)
	rl.DrawText("A minimal, resize-friendly application loop.", 40, 82, 20, rl.GRAY)

	panel := rl.Rectangle{
		x = 40,
		y = 130,
		width = f32(width - 80),
		height = f32(height - 190),
	}
	rl.DrawRectangleRec(panel, rl.LIGHTGRAY)
	rl.DrawRectangleLinesEx(panel, 2, state.accent)

	rl.DrawText("Click the button to open another window.", 64, 160, 22, rl.DARKGRAY)
	click_label := rl.TextFormat("Clicks: %i", state.clicks)
	rl.DrawText(click_label, 64, 200, 28, state.accent)

	draw_main_button()
	rl.DrawFPS(40, height - 40)
}
