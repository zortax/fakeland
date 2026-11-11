# Examples

Runnable programs that use the fakeland library. Each example starts its own
headless session, runs the reference client `fakeland-testclient` and exits on
its own. Arguments after `--` replace the reference client where an example
supports it. Output files go to `target/examples/`.

```sh
just example <name> [args]
# or
cargo build -p fakeland-testclient
cargo run -p fakeland-examples --example <name> -- [args]
```

## Getting started

- `quickstart`: a 144 Hz session at scale 1.25, a screenshot, a scenario and
  the frame pacing preset.
- `run`: runs any command headless for a while and prints the trace summary.
  `just example run --secs 5 -- vkcube --wsi wayland`

## Measurement

- `hover_latency`: input-to-photon latency of a hover with a phase sweep. It
  prints each stage (input to commit, commit to ready, ready to latch, latch
  to present) for a pixel detector and for the app-mark detector. `--hz N`
  sets the refresh rate.
- `display_timing`: the fake display. The same app under fixed 60 and 144 Hz,
  VRR, a perturbed clock with latch jitter and dropped vblanks, and an
  unthrottled output. Then a refresh and scale switch at runtime, and a
  manual output that presents one frame per step.
- `live_trace`: a live subscription to trace records with a once-per-second
  ticker, then exports to Perfetto JSON, a per-frame CSV, JSONL and a JSON
  summary. Open the Perfetto file at <https://ui.perfetto.dev>.
- `record_video`: a 3 s video of the output on the vblank grid (MP4 with
  ffmpeg, else Y4M) and CPU, memory and thread samples of the app.

## Scripting and benches

- `custom_reactor`: a hand-written `Reactor` on the compositor thread. It
  finds long frames of a janky app as they present, marks them in the trace
  and answers each one with input.
- `computer_use`: the agent flow from Rust. It finds a target in a
  downscaled screenshot by color, maps the pixel to logical coordinates,
  clicks, waits on the compositor for the change, types, presses keys and
  uses the clipboard.
- `bench_compare`: one `Bench` with warmup, iterations, a probe and frame
  pacing, run against a smooth app and a janky app. It compares the two
  reports and exits with 1 on a regression.
- `scroll_matrix`: scroll glides over refresh rates, scales and velocities as
  bench variants, and a reactor ping-pong latency loop at 60 and 144 Hz.
  `--quick` runs a smaller matrix. `python3 examples/scroll_matrix_plot.py`
  plots the saved reports into `target/examples/scroll_matrix.png`
  (needs matplotlib).

## Accessibility

- `a11y`: prints the accessibility tree of an app on the private bus and can
  click a node through its action. `just a11y zenity --info`
