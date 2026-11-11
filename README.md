# fakeland

A headless Wayland compositor for benchmarking GUI apps and for driving them
from scripts or AI agents.

## Purpose

- **Benchmarking.** Measure how an app behaves frame by frame: frame times,
  presentation timing and pacing, dropped and late frames, and input-to-photon
  latency (for example, how long a hover takes to show on screen).
- **Scripted and agent-driven use.** Launch an app, take screenshots, move the
  pointer, click, scroll and type, from Rust, from the CLI, over a control
  socket, or through an MCP server for AI agents (GUI development flows,
  headless computer use).

## How it works

fakeland is a real Wayland compositor built on
[smithay](https://github.com/Smithay/smithay). There is no physical display
behind it. The app cannot tell the difference:

- **Real rendering.** The app renders on the GPU into dmabufs (Vulkan or EGL),
  the same as on a desktop compositor. The compositor advertises the usual
  globals and an output with a normal identity.
- **Fake display.** Resolution, scale, refresh rate, VRR and the vblank cadence
  are all configuration. A precise timer on `CLOCK_MONOTONIC` defines the
  vblanks. The compositor latches committed frames, sends presentation feedback
  and frame callbacks, and releases buffers exactly as a hardware display
  pipeline would.
- **Measurement inside the compositor.** Every timestamp comes from the
  compositor thread: input sent, commit, GPU ready, latch, present. Scripted
  input runs as scenarios, triggers or reactors on that thread, so a slow
  driver script never distorts the numbers.
- **Traces and reports.** Sessions record a detailed event trace. Analyzers
  turn it into typed reports (pacing, latency, scroll smoothness, startup,
  resize, idle efficiency, and more), and the trace exports to Perfetto, JSONL
  and CSV.
- **Optional app marks.** Apps can opt in to a private protocol
  (`fakeland-marks`) to mark frames, handled input and internal spans in the
  same timeline, for exact latency attribution.
- **Isolation.** Each session has its own socket, a private D-Bus bus and a
  process group, so apps never reach the host desktop.
- **Headless by default.** A nested viewer window is available to watch the
  session.

Time is wall time: fakeland fakes the display, never the clock. Presentation is
modeled; everything up to the frame being ready is measured. On NVIDIA, GPU
completion times need explicit sync (`linux-drm-syncobj-v1`).

## Quick start

```sh
just build

# Run an app headless at 144 Hz and write a Perfetto trace.
fakeland run --output 2560x1440@144 --scale 1.5 --duration 5s \
    --perfetto run.json -- vkcube --wsi wayland

# Run a bench preset; `fakeland presets` lists them.
fakeland bench hover-latency --report hover.json -- my-app

# Keep a session alive and drive it.
fakeland serve -- alacritty &
fakeland ctl shot out.png
fakeland ctl type "hello"
fakeland ctl stop

# Register the MCP server with Claude Code.
claude mcp add fakeland -- fakeland-mcp
```

For the library API, see the runnable programs in [`examples/`](examples),
starting with `just example quickstart`.

## Layout

A cargo workspace under `crates/`:

- `fakeland`: the compositor library and the `Session` API.
- `fakeland-timing`, `fakeland-trace`, `fakeland-vision`, `fakeland-scenario`,
  `fakeland-types`: pure building blocks (display clock models, traces and
  statistics, frame analysis, the scenario DSL, shared types).
- `fakeland-bench`: benchmark runner, analyzers and presets.
- `fakeland-cli`, `fakeland-control`, `fakeland-mcp`: the `fakeland` command,
  the control socket and the MCP server.
- `fakeland-protocol`, `fakeland-marks`: the app-mark protocol and its client
  library.
- `fakeland-testclient`: reference client for measurement tests.
- `fakeland-a11y`: accessibility tree access.

`examples/` holds runnable examples of the library.
