# fakeland

Headless Wayland compositor for benchmarking and for scripted or agent-driven app interaction. Rust, smithay, cargo workspace. The `fakeland` library starts the compositor, launches apps, fakes the display, drives input, captures frames and records traces. The CLI, the control socket and the MCP server sit on top of it. Planning documents live in `planning/`.

## Workspace
- Crates under `crates/`: `fakeland-types` (shared time, rate, id and geometry types and the session clock), `fakeland-timing` (pure display clock models: fixed, VRR, unthrottled, manual, perturbation, latch and callback policy), `fakeland-trace` (pure trace events, recorder, frame timelines, statistics, exporters), `fakeland-vision` (pure frame analysis: change detection, stability, damage accuracy, motion estimation), `fakeland-scenario` (serde scenario DSL: anchors, actions, input programs, triggers, waits, probes), `fakeland-protocol` (private `fakeland_marks_v1` XML and bindings), `fakeland-marks` (app-side instrumentation client), `fakeland` (the compositor and the `Session` API), `fakeland-bench` (runner, analyzers, reports, comparison), `fakeland-control` (control socket wire types and client), `fakeland-cli` (binary `fakeland`), `fakeland-mcp` (binary `fakeland-mcp`), `fakeland-testclient` (reference client with controllable behavior and ground-truth marks), `fakeland-a11y` (AT-SPI tree access on the private bus). `examples/` is the `fakeland-examples` package: runnable examples with shared helpers in `common.rs`. Each example runs against `fakeland-testclient` by default and writes files to `target/examples`.
- Only `fakeland` names smithay. The pure crates build and test without it. A change to analysis or timing logic must never rebuild smithay.
- Every dependency version lives once in `[workspace.dependencies]`. Crates inherit with `foo.workspace = true`.
- Add crates with `cargo add`. Never write a version from memory. Use the latest version unless a concrete reason forces an older one.
- Crates that use smithay name calloop, rustix, wayland-server, wayland-protocols and xkbcommon through `smithay::reexports`.
- `fakeland-scenario` depends on `fakeland-timing`, so `OutputAction::SetTiming` carries the real `TimingConfig`. Both stay pure.
- Toolchain is pinned in `rust-toolchain.toml` (same nightly as the sibling projects). Do not change it.
- `#![forbid(unsafe_code)]` in every crate.

## Modules and files
- `mod.rs` style for directory modules. `mod.rs` declares and re-exports; it holds no logic. Shared helpers of integration tests live in `tests/support/mod.rs`, so cargo builds no test binary from them.
- Modularize by feature, not by kind. A feature owns its types, state and tests.
- Small files, about 400 lines at most. One responsibility per module. One Wayland protocol handler per file.
- Split a file before it needs a section-separator comment.
- Express the domain in the type system (enums, newtypes, typed ids). Time is `Nanos` and `Instant`-like newtypes on `CLOCK_MONOTONIC`, refresh is `Hz`, coordinates are `Logical` or `Physical`. Stop before the types cost more than they save.
- `thiserror` below the binaries, typed per module. `anyhow` in `main` and in top-level task bodies only.

## Comments and docs
- Short. ASD-STE100 style: one idea per sentence, active voice, present tense.
- Describe what is. Never describe what was there before, what bug was fixed, or why an alternative was rejected.
- No contrastive negation or negative parallelism ("not x, but y", "rather x than y").
- Never mention plans, specs or external documents in code comments or rustdoc.
- Every module starts with a one-paragraph `//!` that states its responsibility.

## Compositor
- The compositor thread owns the calloop event loop and all smithay state. Callers hold a `Send + Sync` `Session` and talk to it through channels. No lock guards compositor state.
- Never block the event loop. Readbacks are bounded to the region a probe needs, and their cost goes into the trace as compositor overhead. Blocking work that would end past the next output wake starts just after that wake: `State::run_when_clear` and `State::clear_slot`. Probe readbacks use them. Screenshots and scenario captures run in steps through `State::capture_then`: render into a reused target, wait for the readback fence on the event loop, then copy the rows in chunks clear of output wakes. `State::capture` blocks until the GPU is done; use it only where a result must exist at once.
- Compositing is lazy: screenshots, probes, the viewer and recording ask for it. `Composite::EveryFrame` is the only mode that composites each vblank.
- A presented buffer stays held until a newer buffer is presented. A replaced buffer is released at the vblank that replaces it.
- Rendering goes through the render node: GBM, EGL, `GlesRenderer`. Pixman and shm serve hosts without a render node.
- Each session owns a socket in `$XDG_RUNTIME_DIR`, a private D-Bus session bus by default, and a process group. Children die with the session.
- Children start through `setpriv --pdeathsig KILL` on the compositor thread, so they also die when the host process dies without cleanup. The private bus starts with a minimal environment in `/`; its activation environment is the child environment, so activated services connect to the session display.
- Extension points on `State`: the timer queue (`schedule_at`, `schedule_in`, `cancel_timer`) shares the vblank timerfd; `observe` sees every trace record after each dispatch; `add_waiter` polls a condition with a deadline; `add_commit_hook` runs at `Committed` (pending state still readable) and at `Applied` (latch). New `Session` methods go in `src/session/*.rs` through `call` or `call_with`, each with a `blocking_*` twin.
- Add a global: one handler file in `src/protocols/` with its delegate macro, a field in `Protocols` (`protocols/state.rs`), creation in `Protocols::new` (`protocols/globals.rs`). Run Chrome, a GTK app and a Qt app against it: a protocol error kills the client.
- Reactors are the Rust path for benches that react to records: `Session::install_reactor` runs a `Reactor` on the compositor thread; its values come back through a `ReactorHandle`.

## Timing and measurement
- Every timestamp is `CLOCK_MONOTONIC` nanoseconds taken on the compositor thread. The send time of an input event is taken after the flush to the client socket.
- Presentation times come from the vblank model. Wake times only measure compositor lateness, and a run whose lateness passes the threshold is invalid.
- Display clock rules live in `fakeland-timing` and are pure. Test them against a fake clock.
- Remote drivers never time anything. Scenarios, triggers and reactors run on the compositor thread; results carry compositor timestamps only.
- Time is read through the session clock only. `clippy.toml` disallows `Instant::now` and `SystemTime::now` elsewhere.
- Trace event variants are append-only. A breaking change bumps `TRACE_FORMAT_VERSION`.
- An analyzer reads a trace and returns a typed, serializable report. It never talks to the compositor.
- A frame is a content commit (`FrameRecord::has_content`: a new buffer or a detach). Frame counts, discards and stage statistics skip commits that only carry state, callbacks or barriers.

## Hiding headless
- An app must see a normal desktop compositor: real-looking output and seat identities, the usual globals, dmabuf feedback with the render node as main device, hardware-clock presentation flags.
- fakeland sets no environment variable and advertises no global that names it. `fakeland_marks_v1` is advertised only when the session enables app marks.
- A new protocol or behavior must keep that illusion.

## Async and state
- The library API is async and runtime agnostic, with `blocking_*` variants. Tokio appears in the binaries only.
- The session shuts down through its command channel, and dropping the last `Session` handle shuts it down too. Scenario runs, probes and waits stop through `util::CancelToken`, one per run.

## Verify
- `just check`, `just test`, `just fmt`, `just lint` (clippy with `-D warnings`, with and without the `viewer` and `nvml` features).
- `just it` builds `fakeland-testclient` and runs the `#[ignore]` integration tests one at a time: sessions drive the reference client and real apps (vkcube, alacritty, zenity, Chrome, designer6) and check measured stages against marks. A test returns early when its program is missing. Tests in one binary share `support::serial()`.
- Integration tests never touch the host desktop: they run with `DISPLAY` and `WAYLAND_DISPLAY` unset, and the viewer tests run only with `FAKELAND_TEST_HOST_DISPLAY` and `FAKELAND_TEST_SWAYSOCK` set to a headless sway the tester starts.
- `just demo <cmd>` runs an app headless and writes a trace; `just viewer <cmd>` runs it with the nested window. `just example <name>` runs an example.
- Test timing models against a fake clock, trace reconstruction against recorded event sequences, vision against synthetic images, scenarios with serde round-trips.
- Commit messages: short subject, active voice, empty body or a few bullets. No references to plans or specs. No attribution trailers.
