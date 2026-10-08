package main

import rl "vendor:raylib"
import "core:os"

SECOND_WINDOW_ARGUMENT :: "--second-window"

is_second_window_mode :: proc(args: []string) -> bool {
	return len(args) > 1 && args[1] == SECOND_WINDOW_ARGUMENT
}

launch_second_window :: proc() {
	// raylib owns one window/context per process, so use a child process for
	// the second independent window.
	process, err := os.process_start(os.Process_Desc{
		command = []string{os.args[0], SECOND_WINDOW_ARGUMENT},
	})
	_ = process
	_ = err
}

run_second_window :: proc() {
	rl.InitWindow(560, 320, "Second raylib window")
	defer rl.CloseWindow()
	rl.SetTargetFPS(60)

	for !rl.WindowShouldClose() {
		rl.BeginDrawing()
		rl.ClearBackground(rl.SKYBLUE)
		rl.DrawText("This is the second window.", 92, 120, 28, rl.WHITE)
		rl.DrawText("Close this window to return to the main one.", 64, 172, 18, rl.DARKBLUE)
		rl.EndDrawing()
	}
}
