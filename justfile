default: check

# Type check every crate in the workspace.
check:
    cargo check --workspace --all-targets

# Run every test.
test:
    cargo test --workspace

# Format every source file.
fmt:
    cargo fmt --all

# Lint every crate, with and without the viewer and nvml features, and treat a warning as an error.
lint:
    cargo clippy --workspace --all-targets -- -D warnings
    cargo clippy --workspace --all-targets --features fakeland/viewer,fakeland/nvml,fakeland-cli/viewer,fakeland-cli/nvml,fakeland-mcp/viewer,fakeland-mcp/nvml -- -D warnings

# Run the integration tests: a session drives the reference client.
it:
    cargo build -p fakeland-testclient
    cargo test --workspace -- --ignored --test-threads=1

# Run an app headless and write a trace.
demo *cmd:
    cargo run -p fakeland-cli -- run --trace target/demo.trace.json -- {{cmd}}

# Run an app with the nested viewer window.
viewer *cmd:
    cargo run -p fakeland-cli --features viewer -- run --viewer -- {{cmd}}

# Build the fakeland, fakeland-mcp and reference client binaries.
build:
    cargo build -p fakeland-cli -p fakeland-mcp -p fakeland-testclient

# Serve a session with a control socket until Ctrl-C, for example `just serve -- alacritty`.
serve *args:
    cargo run -q -p fakeland-cli -- serve {{args}}

# Drive the running session, for example `just ctl click 200 100` or `just ctl windows`.
ctl *args:
    cargo run -q -p fakeland-cli -- ctl {{args}}

# Write a screenshot of the running session, for example `just shot out.png --max-dim 1280`.
shot file="target/shot.png" *args:
    cargo run -q -p fakeland-cli -- ctl shot {{file}} {{args}}

# Run an app for a time and write its JSONL and Perfetto traces, for example `just trace 5s vkcube`.
trace duration *cmd:
    cargo run -q -p fakeland-cli -- run --duration {{duration}} --trace target/run.trace.jsonl --perfetto target/run.perfetto.json -- {{cmd}}

# Run the MCP server on stdio.
mcp:
    cargo run -q -p fakeland-mcp

# Bench an app with a preset or scenario file, for example `just bench hover-latency alacritty`.
bench preset *cmd:
    cargo run -q --release -p fakeland-cli -- bench {{preset}} --report target/bench-{{file_stem(preset)}}.json --perfetto target/bench-{{file_stem(preset)}}.perfetto.json -- {{cmd}}

# List the bench presets, or show the parameters of one.
presets *name:
    cargo run -q -p fakeland-cli -- presets {{name}}

# Chromium and Electron need --force-renderer-accessibility for web content.
# Run an app headless and print its accessibility tree, for example `just a11y zenity --info`.
a11y *cmd:
    cargo run -q -p fakeland-examples --example a11y -- -- {{cmd}}

# Record a video of an app for a time, for example `just record 5s target/out.mp4 vkcube`.
record duration file *cmd:
    cargo run -q -p fakeland-cli -- run --duration {{duration}} --record {{file}} --sample 100ms -- {{cmd}}

# Run an example of `examples/`, for example `just example hover_latency`.
example name *args:
    cargo build -q -p fakeland-testclient
    cargo run -q -p fakeland-examples --example {{name}} -- {{args}}
