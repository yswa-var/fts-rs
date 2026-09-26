package main

import rl "vendor:raylib"

MAIN_BUTTON :: rl.Rectangle{x = 64, y = 260, width = 240, height = 56}

main_button_was_clicked :: proc() -> bool {
	return rl.IsMouseButtonPressed(.LEFT) &&
		rl.CheckCollisionPointRec(rl.GetMousePosition(), MAIN_BUTTON)
}

draw_main_button :: proc() {
	color := rl.DARKBLUE
	if rl.CheckCollisionPointRec(rl.GetMousePosition(), MAIN_BUTTON) {
		color = rl.BLUE
	}

	rl.DrawRectangleRec(MAIN_BUTTON, color)
	rl.DrawText("Open second window", 88, 278, 18, rl.WHITE)
}
