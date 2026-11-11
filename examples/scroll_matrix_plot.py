#!/usr/bin/env python3
"""Plots the reports of the scroll_matrix example.

Run `just example scroll_matrix` first. This script reads
target/examples/scroll_matrix*.json and writes target/examples/scroll_matrix.png.
It needs matplotlib. An optional argument names another report directory.
"""

import json
import re
import sys
from pathlib import Path

import matplotlib

matplotlib.use("Agg")
import matplotlib.pyplot as plt  # noqa: E402

SURFACE = "#fcfcfb"
TEXT = "#0b0b0b"
TEXT_2 = "#52514e"
GRID = "#e4e3df"
# One fixed color per refresh rate, in categorical slot order.
RATE_COLORS = {60: "#2a78d6", 120: "#eb6834", 144: "#1baf7a"}
VARIANT = re.compile(r"(?P<hz>[\d.]+)Hz x(?P<scale>[\d.]+) (?P<v>[\d.]+)px/s")


def load(directory: Path):
    matrix = json.loads((directory / "scroll_matrix.json").read_text())
    ping = {
        hz: json.loads((directory / f"scroll_matrix-pingpong-{hz}.json").read_text())
        for hz in (60, 144)
        if (directory / f"scroll_matrix-pingpong-{hz}.json").exists()
    }
    return matrix, ping


def parse(variant: str):
    m = VARIANT.fullmatch(variant or "")
    if not m:
        return None
    return int(float(m["hz"])), float(m["scale"]), int(float(m["v"]))


def section(report, name):
    return next(s for s in report["sections"] if s["name"] == name)


def metric(sec, name):
    """Returns {(hz, scale, velocity): median} for one metric."""
    out = {}
    for m in sec["metrics"]:
        key = parse(m.get("variant"))
        if m["name"] == name and key:
            out[key] = m["median"]
    return out


def style(ax, title, ylabel):
    ax.set_facecolor(SURFACE)
    ax.set_title(title, loc="left", color=TEXT, fontsize=11, fontweight="bold", pad=10)
    ax.set_ylabel(ylabel, color=TEXT_2, fontsize=9)
    ax.tick_params(colors=TEXT_2, labelsize=8, length=0)
    ax.grid(axis="y", color=GRID, linewidth=0.8)
    ax.set_axisbelow(True)
    for side in ("top", "right", "left"):
        ax.spines[side].set_visible(False)
    ax.spines["bottom"].set_color(GRID)


def grouped_bars(ax, values, rates, conditions, fmt):
    """Bars per condition, one bar per rate, with the value above each bar."""
    width = 0.8 / len(rates)
    for i, hz in enumerate(rates):
        xs, ys = [], []
        for j, cond in enumerate(conditions):
            v = values.get((hz, *cond))
            if v is not None:
                xs.append(j + (i - (len(rates) - 1) / 2) * width)
                ys.append(v)
        bars = ax.bar(
            xs, ys, width, color=RATE_COLORS[hz], edgecolor=SURFACE, linewidth=2,
            label=f"{hz} Hz",
        )
        ax.bar_label(bars, labels=[fmt(y) for y in ys], fontsize=7, color=TEXT_2, padding=2)
    ax.set_xticks(range(len(conditions)))
    ax.set_xticklabels([f"{v} px/s\nscale {s:g}" for s, v in conditions])


def velocity_trace(axes, sec, rates, velocity):
    """Content velocity per presented frame of one glide, one small chart per rate."""
    for ax, hz in zip(axes, rates):
        it = next(
            (
                i for i in sec["iterations"]
                if parse(i["variant"]) == (hz, 1.0, velocity)
            ),
            None,
        )
        if it is None:
            continue
        steps = [s for s in it["report"]["steps"] if s["active"] and s.get("presented")]
        if not steps:
            continue
        t0 = steps[0]["presented"]
        ts, vs = [], []
        for s in steps:
            if s["measured_px"] is None or s["interval_ms"] <= 0:
                continue
            ts.append((s["presented"] - t0) / 1e6)
            vs.append(abs(s["measured_px"]) / s["interval_ms"] * 1000)
        ax.axhline(velocity, color=TEXT_2, linewidth=1, linestyle=(0, (4, 3)))
        ax.plot(ts, vs, color=RATE_COLORS[hz], linewidth=1.5, marker="o", markersize=3)
        ax.set_ylim(0, velocity * 1.75)
        ax.set_yticks([0, velocity])
        ax.text(0.005, 0.92, f"{hz} Hz", transform=ax.transAxes, fontsize=9, color=TEXT,
                fontweight="bold", va="top")
    axes[0].annotate(
        f"dashed: input {velocity} px/s", xy=(1, velocity), xycoords=("axes fraction", "data"),
        xytext=(0, -5), textcoords="offset points", ha="right", va="top", fontsize=8,
        color=TEXT_2,
    )
    for ax in axes[:-1]:
        ax.tick_params(labelbottom=False)
    axes[-1].set_xlabel("time since the first moving frame (ms)", color=TEXT_2, fontsize=9)


def ping_pong(ax, ping):
    """Input-to-photon latency of each ping-pong iteration."""
    labels, positions = [], []
    for x, (hz, report) in enumerate(sorted(ping.items())):
        sec = section(report, "ping_pong")
        totals = [i["report"]["total_ms"] for i in sec["iterations"] if i["report"].get("total_ms")]
        for k, total in enumerate(totals):
            xi = x + (k - (len(totals) - 1) / 2) * 0.07
            ax.vlines(xi, total["min"], total["max"], color=RATE_COLORS[hz], linewidth=3,
                      alpha=0.4)
            ax.plot(xi, total["p50"], "o", color=RATE_COLORS[hz], markersize=8,
                    markeredgecolor=SURFACE, markeredgewidth=2)
        p50 = next(m["median"] for m in sec["metrics"] if m["name"] == "total_p50")
        ax.annotate(f"p50 {p50:.1f} ms", xy=(x + 0.2, p50), xytext=(6, 0),
                    textcoords="offset points", va="center", fontsize=8, color=TEXT_2)
        labels.append(f"{hz} Hz")
        positions.append(x)
    ax.set_xticks(positions)
    ax.set_xticklabels(labels)
    ax.set_xlim(-0.6, len(positions) - 0.2)
    ax.set_ylim(bottom=0)


def main():
    directory = (
        Path(sys.argv[1]) if len(sys.argv) > 1
        else Path(__file__).resolve().parent.parent / "target" / "examples"
    )
    matrix, ping = load(directory)
    scroll = section(matrix, "scroll_smoothness")
    keys = {k for k in metric(scroll, "smoothness_score")}
    rates = sorted({k[0] for k in keys})
    conditions = sorted({(k[1], k[2]) for k in keys}, key=lambda c: (c[1], c[0]))
    fastest = max(v for _, v in conditions)

    plt.rcParams["font.family"] = "DejaVu Sans"
    fig = plt.figure(figsize=(13, 8.5), facecolor=SURFACE)
    grid = fig.add_gridspec(2, 2, left=0.06, right=0.98, top=0.86, bottom=0.08,
                            hspace=0.45, wspace=0.2)
    a, c, d = fig.add_subplot(grid[0, 0]), fig.add_subplot(grid[1, 0]), fig.add_subplot(grid[1, 1])
    trace = grid[0, 1].subgridspec(len(rates), 1, hspace=0.12)
    b = [fig.add_subplot(trace[i]) for i in range(len(rates))]
    for ax in b[1:]:
        ax.sharex(b[0])

    grouped_bars(a, metric(scroll, "smoothness_score"), rates, conditions, lambda y: f"{y:.0f}")
    style(a, "Scroll smoothness", "score (0 to 100, higher is smoother)")
    a.set_ylim(0, 110)

    velocity_trace(b, scroll, rates, fastest)
    for ax in b:
        style(ax, "", "")
    b[0].set_title(f"Content velocity per frame, {fastest} px/s glide at scale 1", loc="left",
                   color=TEXT, fontsize=11, fontweight="bold", pad=10)
    b[len(b) // 2].set_ylabel("px/s", color=TEXT_2, fontsize=9)

    grouped_bars(c, metric(scroll, "start_latency"), rates, conditions, lambda y: f"{y:.1f}")
    style(c, "Scroll start latency", "first scroll event to first moved frame (ms)")

    if ping:
        ping_pong(d, ping)
        style(d, "Hover ping-pong, input to photon", "ms (dot: iteration p50, bar: min to max)")
    else:
        d.set_visible(False)

    handles, labels = a.get_legend_handles_labels()
    fig.legend(handles, labels, loc="upper right", ncol=len(rates), frameon=False,
               fontsize=9, labelcolor=TEXT, bbox_to_anchor=(0.98, 0.985))
    env = matrix.get("environment", {})
    fig.suptitle(
        "fakeland scroll matrix", x=0.02, y=0.99, ha="left", color=TEXT, fontsize=14,
        fontweight="bold",
    )
    fig.text(0.02, 0.935, f"{env.get('gpu', '')} · {env.get('renderer', '')} · reference client",
             color=TEXT_2, fontsize=9)
    out = directory / "scroll_matrix.png"
    fig.savefig(out, dpi=150, facecolor=SURFACE)
    print(out)


if __name__ == "__main__":
    main()
