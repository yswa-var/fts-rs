package main

import "core:os"

main :: proc() {
	if is_second_window_mode(os.args) {
		run_second_window()
		return
	}
	run_main_window()
}
