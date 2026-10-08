# Odin + raylib starter

This directory contains a compact raylib primitive-node editor split into small
Odin source files:

- `main.odin` — program entry point and window-mode selection.
- `app.odin` — editor state, camera, grid, nodes, update, and drawing.
- `ui.odin` — toolbar, spawn button, hit testing, and editor instructions.
- `windows.odin` — second-window rendering and child-process launching.

The project also retains the second-window process example in `windows.odin`
for future editor panels or auxiliary tools.

The editor currently supports:

- Spawn an RSI node with **Spawn RSI** or the **Space** key.
- Select and move nodes with the left mouse button.
- Pan the infinite canvas with the middle mouse button.
- Zoom around the cursor with the mouse wheel.

Node machinery is data-driven: `Node_Definition` describes the RSI title,
dimensions, typed ports, and port counts; `Node_Instance` adds the stable ID
and world position; the generic renderer draws every node from that definition.
The current RSI definition exposes a `Series` input, an `Integer` period input,
and a `Series` RSI output. These definitions are used by the generic renderer
and are ready for the next series-engine milestone.

The graph logistics slice is now present:

- Ports have `Float`, `Integer`, `Series`, or `Bars` types.
- Drag from an output port to an input port to create a typed edge.
- Incompatible connections are rejected and each edge is drawn as a colored belt.
- `Run` or `R` performs a topological evaluation.
- The seeded graph evaluates `10 + 20` through `Add` into `Probe` and displays `OUTPUT = 30`.

`Market Bars`, `Close`, `SMA`, and `RSI` definitions are ready for the series
engine; their domain evaluators will be added with historical market data.

From this directory, build and run it with:

```bash
odin run . -out:raylib-starter
```

Odin imports its bundled bindings through `vendor:raylib`. Install raylib's
native development library through your system package manager if linking fails
(on Debian/Ubuntu: `sudo apt install libraylib-dev`).
