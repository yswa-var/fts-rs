# Odin + raylib starter

This directory contains a compact raylib application skeleton split into small
Odin source files:

- `main.odin` — program entry point and window-mode selection.
- `app.odin` — main-window lifecycle, application state, update, and drawing.
- `ui.odin` — button geometry, hit testing, and button rendering.
- `windows.odin` — second-window rendering and child-process launching.

The main window demonstrates a button that launches a second independent
raylib window by starting the same executable in a child process.

From this directory, build and run it with:

```bash
odin run . -out:raylib-starter
```

Click **Open second window** in the main window. Close the child window
independently; the main window stays open.

Odin imports its bundled bindings through `vendor:raylib`. Install raylib's
native development library through your system package manager if linking fails
(on Debian/Ubuntu: `sudo apt install libraylib-dev`).
